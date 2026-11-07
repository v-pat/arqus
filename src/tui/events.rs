//! Folding agent events into the transcript. Pure: no drawing, no I/O.

use crate::agent::AgentEvent;

use super::app::{App, LineKind, UiLine};

impl App {
    /// Fold one agent event into the transcript.
    pub fn apply_event(&mut self, ev: AgentEvent) {
        match ev {
            AgentEvent::TextDelta(text) => {
                self.status = "streaming".to_string();
                self.thinking_line = None;
                self.assistant_line =
                    self.append_to(LineKind::Assistant, self.assistant_line, &text);
            }
            AgentEvent::ThinkingDelta(text) => {
                self.assistant_line = None;
                self.thinking_line = self.append_to(LineKind::Thinking, self.thinking_line, &text);
            }
            AgentEvent::ToolStart { name, args } => {
                self.assistant_line = None;
                self.thinking_line = None;
                let args = compact_args(&args);
                self.push(LineKind::ToolCall, format!("-> {name} {args}"));
                self.status = format!("tool: {name}");
            }
            AgentEvent::ToolEnd {
                name,
                text,
                is_error,
            } => {
                self.assistant_line = None;
                self.thinking_line = None;
                self.push_result(&name, &text, is_error);
                self.status = "waiting for model".to_string();
            }
            AgentEvent::Compacted { tokens_before } => {
                if tokens_before > 0 {
                    self.context_tokens = tokens_before;
                }
                self.push(
                    LineKind::Info,
                    format!("[compacted conversation: ~{tokens_before} tokens before]"),
                );
            }
            AgentEvent::Notice(text) => {
                self.push(LineKind::Info, format!("[pirs] {text}"));
                self.status = text;
            }
            AgentEvent::UserMessage(text) => {
                self.dequeue(&text);
                self.push_user(&text);
            }
            AgentEvent::ApprovalRequest { tool, preview } => {
                self.approval_request(tool, preview);
            }
            AgentEvent::Error(text) => {
                self.push(LineKind::Error, format!("error: {text}"));
                self.status = "error".to_string();
            }
            AgentEvent::TurnDone => {
                self.running = false;
                self.assistant_line = None;
                self.thinking_line = None;
                self.approval = None;
                self.status = "ready".to_string();
            }
        }
    }

    pub(crate) fn push(&mut self, kind: LineKind, text: String) {
        self.lines.push(UiLine {
            kind,
            text,
            dim: false,
        });
        self.refresh_wrap();
        self.follow_bottom();
    }

    fn append_to(&mut self, kind: LineKind, index: Option<usize>, text: &str) -> Option<usize> {
        if let Some(i) = index {
            if self.lines.get(i).map(|l| l.kind == kind).unwrap_or(false) {
                self.lines[i].text.push_str(text);
                self.refresh_wrap();
                self.follow_bottom();
                return Some(i);
            }
        }
        self.push(kind, text.to_string());
        self.lines.len().checked_sub(1)
    }

    fn push_result(&mut self, name: &str, text: &str, is_error: bool) {
        let kind = if is_error {
            LineKind::Error
        } else {
            LineKind::ToolResult
        };
        let head = if is_error {
            format!("<- {name} [error]: ")
        } else {
            format!("<- {name}: ")
        };
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            self.push(kind, head);
            return;
        }
        for (i, line) in lines.iter().take(10).enumerate() {
            let prefix = if i == 0 {
                head.clone()
            } else {
                "   ".to_string()
            };
            self.push(kind, format!("{prefix}{line}"));
        }
        if lines.len() > 10 {
            self.push(kind, format!("   ... {} more lines", lines.len() - 10));
        }
    }
}

fn compact_args(args: &serde_json::Value) -> String {
    let text = serde_json::to_string(args).unwrap_or_default();
    let mut out: String = text.chars().take(120).collect();
    if text.chars().count() > 120 {
        out.push_str("...");
    }
    out
}
