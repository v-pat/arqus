//! The agent loop. Turns one user prompt into model calls and tool runs.
//!
//! The core knows nothing about the UI: it sends `AgentEvent` values through
//! a channel. Print mode and the TUI are two consumers of that channel.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc::Sender;

use crate::ai::{Llm, LlmRequest, StreamEvent};
use crate::compaction::{compact, context_tokens, should_compact, CompactionSettings};
use crate::context::build_context;
use crate::session::Session;
use crate::tools;
use crate::types::{Block, EntryKind, Message, StopReason, Usage};

/// One event from a turn. The UI decides how to show it.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolStart {
        name: String,
        args: Value,
    },
    ToolEnd {
        name: String,
        text: String,
        is_error: bool,
    },
    Compacted {
        tokens_before: u64,
    },
    TurnDone,
    Error(String),
}

/// Everything one turn needs that is not in the session.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub cwd: PathBuf,
    pub system_prompt: String,
    pub context_window: u64,
    pub compaction: CompactionSettings,
}

#[path = "runtime.rs"]
mod runtime;
pub use runtime::Runtime;

const MAX_ROUNDS: usize = 50;

/// Run one user prompt to completion, appending entries to the session.
pub async fn run_prompt(
    session: &mut Session,
    llm: &Llm,
    cfg: &AgentConfig,
    user_text: String,
    events: Sender<AgentEvent>,
) -> Result<()> {
    session.append(EntryKind::Message {
        message: Message::User { text: user_text },
    })?;

    let mut overflow_retried = false;

    for _round in 0..MAX_ROUNDS {
        let messages = build_context(&session.entries, session.leaf_id.as_deref());
        let request = LlmRequest {
            system: cfg.system_prompt.clone(),
            messages,
            tools: tools::tool_defs(),
            max_tokens: None,
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel(256);
        let task = tokio::spawn({
            let llm = llm.clone();
            async move { llm.stream(request, tx).await }
        });

        let mut text = String::new();
        let mut thinking = String::new();
        let mut done: Option<Message> = None;
        let mut stream_error: Option<String> = None;

        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::TextDelta(delta) => {
                    text.push_str(&delta);
                    let _ = events.send(AgentEvent::TextDelta(delta)).await;
                }
                StreamEvent::ThinkingDelta(delta) => {
                    thinking.push_str(&delta);
                    let _ = events.send(AgentEvent::ThinkingDelta(delta)).await;
                }
                // The Done message already carries the complete blocks.
                StreamEvent::ToolCall { .. } => {}
                StreamEvent::Done(message) => done = Some(message),
                StreamEvent::Error(text) => {
                    stream_error.get_or_insert(text);
                }
            };
        }

        if let Err(err) = task.await? {
            stream_error.get_or_insert_with(|| format!("{err:#}"));
        }

        if let Some(err) = stream_error {
            // Overflow recovery: compact once, then retry this request once.
            if !overflow_retried && is_context_overflow(&err) {
                overflow_retried = true;
                if compact(session, llm, &cfg.compaction, None).await? {
                    let _ = events
                        .send(AgentEvent::Compacted { tokens_before: 0 })
                        .await;
                    continue;
                }
            }
            let (provider, model, usage, blocks) = parts(done, &text, &thinking);
            session.append(EntryKind::Message {
                message: Message::Assistant {
                    blocks,
                    provider,
                    model,
                    usage,
                    stop: StopReason::Error,
                },
            })?;
            let _ = events.send(AgentEvent::Error(err)).await;
            let _ = events.send(AgentEvent::TurnDone).await;
            return Ok(());
        }

        let blocks = match &done {
            Some(Message::Assistant { blocks, .. }) => blocks.clone(),
            _ => accumulated(&text, &thinking),
        };

        let message = match done {
            Some(message @ Message::Assistant { .. }) => message,
            _ => Message::Assistant {
                blocks: blocks.clone(),
                provider: "unknown".to_string(),
                model: String::new(),
                usage: None,
                stop: StopReason::Stop,
            },
        };
        session.append(EntryKind::Message { message })?;

        let calls: Vec<(String, String, Value)> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::ToolCall { id, name, args } => {
                    Some((id.clone(), name.clone(), args.clone()))
                }
                _ => None,
            })
            .collect();

        if calls.is_empty() {
            maybe_compact(session, llm, cfg, &events).await?;
            break;
        }

        for (id, name, args) in calls {
            let _ = events
                .send(AgentEvent::ToolStart {
                    name: name.clone(),
                    args: args.clone(),
                })
                .await;
            let output = tools::execute(&name, &args, &cfg.cwd).await;
            session.append(EntryKind::Message {
                message: Message::ToolResult {
                    call_id: id,
                    tool_name: name.clone(),
                    text: output.text.clone(),
                    is_error: output.is_error,
                },
            })?;
            let _ = events
                .send(AgentEvent::ToolEnd {
                    name,
                    text: output.text,
                    is_error: output.is_error,
                })
                .await;
        }

        maybe_compact(session, llm, cfg, &events).await?;
    }

    let _ = events.send(AgentEvent::TurnDone).await;
    Ok(())
}

/// Compact when the context no longer fits. Never fails the turn on a
/// summarizer error: the next request reports the real problem.
async fn maybe_compact(
    session: &mut Session,
    llm: &Llm,
    cfg: &AgentConfig,
    events: &Sender<AgentEvent>,
) -> Result<()> {
    if !cfg.compaction.enabled {
        return Ok(());
    }
    let messages = build_context(&session.entries, session.leaf_id.as_deref());
    let tokens = context_tokens(&messages);
    if !should_compact(tokens, cfg.context_window, cfg.compaction.reserve_tokens) {
        return Ok(());
    }
    if compact(session, llm, &cfg.compaction, None).await? {
        let _ = events
            .send(AgentEvent::Compacted {
                tokens_before: tokens,
            })
            .await;
    }
    Ok(())
}

/// Best-effort detection of a provider context-length rejection.
fn is_context_overflow(message: &str) -> bool {
    let text = message.to_lowercase();
    let names_context =
        text.contains("context") && (text.contains("length") || text.contains("token"));
    names_context || text.contains("400") || text.contains("413")
}

/// Provider, model, usage and blocks of a finished (or failed) assistant message.
fn parts(
    done: Option<Message>,
    text: &str,
    thinking: &str,
) -> (String, String, Option<Usage>, Vec<Block>) {
    match done {
        Some(Message::Assistant {
            blocks,
            provider,
            model,
            usage,
            ..
        }) => (provider, model, usage, blocks),
        _ => (
            "unknown".to_string(),
            String::new(),
            None,
            accumulated(text, thinking),
        ),
    }
}

/// Rebuild assistant blocks from streamed deltas when no Done message arrived.
fn accumulated(text: &str, thinking: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    if !text.is_empty() {
        blocks.push(Block::Text {
            text: text.to_string(),
        });
    }
    if !thinking.is_empty() {
        blocks.push(Block::Thinking {
            text: thinking.to_string(),
        });
    }
    blocks
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
