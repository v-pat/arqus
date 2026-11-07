//! Small pure helpers for the agent loop: reply assembly and runaway checks.

use serde_json::Value;

use crate::types::{Block, Message, Usage};

/// Best-effort detection of a provider context-length rejection.
pub fn is_context_overflow(message: &str) -> bool {
    let text = message.to_lowercase();
    let names_context =
        text.contains("context") && (text.contains("length") || text.contains("token"));
    names_context || text.contains("400") || text.contains("413")
}

/// Provider, model, usage and blocks of a finished (or failed) assistant message.
pub fn parts(
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
pub fn accumulated(text: &str, thinking: &str) -> Vec<Block> {
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

/// Detects the two runaway shapes from the stage rules.
#[derive(Debug, Default)]
pub struct Runaway {
    last_call: Option<(String, Value)>,
    repeated: usize,
    previous_empty: bool,
}

impl Runaway {
    /// Record one assistant reply. `true` means two empty replies in a row:
    /// stop the turn.
    pub fn empty_twice(&mut self, has_content: bool) -> bool {
        if has_content {
            self.previous_empty = false;
            return false;
        }
        if self.previous_empty {
            return true;
        }
        self.previous_empty = true;
        false
    }

    /// Record one tool call. `true` allows it; `false` means the same call
    /// arrived three times in a row, so the turn must stop.
    pub fn allow_call(&mut self, name: &str, args: &Value) -> bool {
        let signature = (name.to_string(), args.clone());
        if self.last_call.as_ref() == Some(&signature) {
            self.repeated += 1;
        } else {
            self.repeated = 1;
            self.last_call = Some(signature);
        }
        self.repeated < 3
    }
}

/// Append every message waiting in the steering channel, in order.
/// Returns how many user entries were added.
pub fn drain_steering(
    session: &mut crate::session::Session,
    steer_rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
    events: &tokio::sync::mpsc::Sender<super::AgentEvent>,
) -> anyhow::Result<usize> {
    let mut count = 0;
    while let Ok(text) = steer_rx.try_recv() {
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        session.append(crate::types::EntryKind::Message {
            message: Message::User { text: text.clone() },
        })?;
        let _ = events.try_send(super::AgentEvent::UserMessage(text));
        count += 1;
    }
    Ok(count)
}

/// Compact when the context no longer fits. Never fails the turn on a
/// summarizer error: the next request reports the real problem.
pub async fn maybe_compact(
    session: &mut crate::session::Session,
    llm: &crate::ai::Llm,
    cfg: &super::AgentConfig,
    events: &tokio::sync::mpsc::Sender<super::AgentEvent>,
) -> anyhow::Result<()> {
    if !cfg.compaction.enabled {
        return Ok(());
    }
    let messages = crate::context::build_context(&session.entries, session.leaf_id.as_deref());
    let tokens = crate::compaction::context_tokens(&messages);
    if !crate::compaction::should_compact(tokens, cfg.context_window, cfg.compaction.reserve_tokens)
    {
        return Ok(());
    }
    if crate::compaction::compact(session, llm, &cfg.compaction, None).await? {
        let _ = events
            .send(super::AgentEvent::Compacted {
                tokens_before: tokens,
            })
            .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn third_identical_call_is_refused() {
        let mut runaway = Runaway::default();
        assert!(runaway.allow_call("bash", &json!({"command": "ls"})));
        assert!(runaway.allow_call("bash", &json!({"command": "ls"})));
        assert!(!runaway.allow_call("bash", &json!({"command": "ls"})));
    }

    #[test]
    fn a_different_call_resets_the_count() {
        let mut runaway = Runaway::default();
        assert!(runaway.allow_call("bash", &json!({"command": "ls"})));
        assert!(runaway.allow_call("bash", &json!({"command": "ls"})));
        assert!(runaway.allow_call("bash", &json!({"command": "pwd"})));
        assert!(runaway.allow_call("bash", &json!({"command": "pwd"})));
        assert!(!runaway.allow_call("bash", &json!({"command": "pwd"})));
    }

    #[test]
    fn two_empty_replies_in_a_row_stop() {
        let mut runaway = Runaway::default();
        assert!(!runaway.empty_twice(false));
        assert!(runaway.empty_twice(false));
    }

    #[test]
    fn content_resets_the_empty_counter() {
        let mut runaway = Runaway::default();
        assert!(!runaway.empty_twice(false));
        assert!(!runaway.empty_twice(true));
        assert!(!runaway.empty_twice(false));
    }
}
