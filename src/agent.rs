//! The agent loop. Turns one user prompt into model calls and tool runs.
//!
//! The core knows nothing about the UI: it sends `AgentEvent` values through
//! a channel. Print mode and the TUI are two consumers of that channel.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc::{Sender, UnboundedReceiver};

use crate::ai::{Llm, LlmRequest, StreamEvent};
use crate::compaction::{compact, CompactionSettings};
use crate::context::build_context;
use crate::cost::{self, Limits};
use crate::safety::{Approvals, Approver, Decision, Guard};
use crate::session::Session;
use crate::tools;
use crate::types::{Block, EntryKind, Message, StopReason};

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
    /// Progress line from the LLM layer, for example before a retry.
    Notice(String),
    /// A queued steering message was accepted into the session.
    UserMessage(String),
    /// The agent is waiting for a y/a/n answer in the TUI.
    ApprovalRequest {
        tool: String,
        preview: String,
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
    pub limits: Limits,
    pub approver: Approver, // approvals, path rules and the risky switch
    pub allow_risky: bool,
    /// True in plan mode: write, edit and bash return `PLAN_REFUSAL`.
    pub plan: bool,
    pub approvals: Approvals,
    pub guard: Guard,
}

impl AgentConfig {
    /// The system prompt for one request. Plan mode adds one line.
    pub fn turn_system_prompt(&self) -> String {
        if self.plan {
            format!("{}\n{}", self.system_prompt, PLAN_NOTE)
        } else {
            self.system_prompt.clone()
        }
    }
}

#[path = "runtime.rs"]
mod runtime;
pub use runtime::Runtime;

#[path = "agent_safety.rs"]
mod safety_flow;
#[path = "agent_util.rs"]
mod util;
pub use safety_flow::PLAN_NOTE;
#[cfg(test)]
pub use safety_flow::{preview as approval_preview, PLAN_REFUSAL};
#[cfg(test)]
use util::drain_steering;
use util::{accumulated, is_context_overflow, parts, Runaway};
/// Run one user prompt to completion, appending entries to the session.
/// `steer_rx` delivers typed messages; `approval_rx` answers the TUI modal.
pub async fn run_prompt(
    session: &mut Session,
    llm: &Llm,
    cfg: &AgentConfig,
    user_text: String,
    events: Sender<AgentEvent>,
    mut steer_rx: UnboundedReceiver<String>,
    mut approval_rx: tokio::sync::mpsc::Receiver<Decision>,
) -> Result<()> {
    session.append(EntryKind::Message {
        message: Message::User { text: user_text },
    })?;

    let mut overflow_retried = false;
    let run_start = cost::session_totals(&session.entries);
    let mut runaway = Runaway::default();

    for _round in 0..cfg.limits.max_rounds {
        // Steering: anything typed during the last tool round joins the
        // history before the next request.
        util::drain_steering(session, &mut steer_rx, &events)?;

        let session_totals = cost::session_totals(&session.entries);
        let run_totals = session_totals.since(&run_start);
        if let Some(message) = cost::check_limits(&session_totals, &run_totals, &cfg.limits) {
            let _ = events.send(AgentEvent::TurnDone).await;
            return Err(anyhow::anyhow!(message));
        }

        let messages = build_context(&session.entries, session.leaf_id.as_deref());
        let request = LlmRequest {
            system: cfg.turn_system_prompt(),
            messages,
            tools: tools::tool_defs(),
            max_tokens: Some(cfg.limits.max_output_tokens),
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
        let mut overflow = false;

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
                    overflow = overflow || is_context_overflow(&text);
                    stream_error.get_or_insert(text);
                }
                StreamEvent::Notice(text) => {
                    let _ = events.send(AgentEvent::Notice(text)).await;
                }
            };
        }

        if let Err(err) = task.await? {
            overflow = overflow
                || err
                    .downcast_ref::<crate::ai::LlmError>()
                    .map(crate::ai::LlmError::is_context_overflow)
                    .unwrap_or(false)
                || is_context_overflow(&format!("{err:#}"));
            stream_error.get_or_insert_with(|| crate::ai::friendly_error(&err));
        }

        if let Some(err) = stream_error {
            // Overflow recovery: compact once, then retry this request once.
            if !overflow_retried && overflow {
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
            crate::debug::log("error", serde_json::json!({"text": err}));
            let _ = events.send(AgentEvent::TurnDone).await;
            // Print mode turns this into exit code 1. The session already
            // holds the partial assistant message with `stop: error`.
            return Err(anyhow::anyhow!("{err}"));
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

        let has_content = blocks.iter().any(|block| match block {
            Block::Text { text } => !text.is_empty(),
            Block::ToolCall { .. } => true,
            Block::Thinking { .. } => false,
        });

        if runaway.empty_twice(has_content) {
            let _ = events
                .send(AgentEvent::Notice(
                    "stopped: two assistant messages had no text and no tool calls".to_string(),
                ))
                .await;
            break;
        }
        if !has_content && calls.is_empty() {
            // Give the model one more chance; a second empty reply stops.
            continue;
        }

        if calls.is_empty() {
            util::maybe_compact(session, llm, cfg, &events).await?;
            // Leftover steering starts a new turn instead of ending the run.
            if util::drain_steering(session, &mut steer_rx, &events)? > 0 {
                continue;
            }
            break;
        }

        let mut stopped = false;
        for (id, name, args) in calls {
            // Runaway protection: the same call three times in a row.
            if !runaway.allow_call(&name, &args) {
                let _ = events
                    .send(AgentEvent::Notice(
                        "stopped: repeated the same tool call 3 times".to_string(),
                    ))
                    .await;
                stopped = true;
                break;
            }

            let _ = events
                .send(AgentEvent::ToolStart {
                    name: name.clone(),
                    args: args.clone(),
                })
                .await;
            crate::debug::log("tool", serde_json::json!({"name": name, "args": args}));
            let output =
                match safety_flow::approve(&name, &args, cfg, &events, &mut approval_rx).await {
                    Ok(()) => tools::execute(&name, &args, &cfg.cwd).await,
                    Err(refusal) => refusal,
                };
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

        if stopped {
            let _ = events.send(AgentEvent::TurnDone).await;
            return Ok(());
        }

        util::maybe_compact(session, llm, cfg, &events).await?;
    }

    let _ = events.send(AgentEvent::TurnDone).await;
    Ok(())
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
