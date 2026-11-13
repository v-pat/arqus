//! Falling agent events into transcript blocks. Pure: no drawing, no I/O.

use crate::agent::AgentEvent;

use super::app::App;
use super::block::{summary_for, Block, ToolBlock, ToolState};
use super::msg::Level;

impl App {
    /// Fold one agent event into the transcript. True means: ring the bell.
    pub fn apply_event(&mut self, ev: AgentEvent) -> bool {
        let mut bell = false;
        match ev {
            AgentEvent::TextDelta(text) => {
                self.status = "streaming".to_string();
                self.retry_notice = None;
                match self.blocks.last_mut() {
                    Some(Block::Assistant {
                        text: current,
                        streaming: true,
                    }) => current.push_str(&text),
                    _ => self.blocks.push(Block::Assistant {
                        text,
                        streaming: true,
                    }),
                }
                self.refresh_render();
                self.follow_bottom();
            }
            AgentEvent::ThinkingDelta(text) => {
                match self.blocks.last_mut() {
                    Some(Block::Thinking { text: current }) => current.push_str(&text),
                    _ => self.blocks.push(Block::Thinking { text }),
                }
                self.refresh_render();
                self.follow_bottom();
            }
            AgentEvent::ToolStart { name, args } => {
                self.streaming_done();
                let id = self.blocks.len();
                self.blocks
                    .push(Block::Tool(ToolBlock::new(id, &name, &args)));
                self.status = format!("tool: {name}");
                self.refresh_render();
                self.follow_bottom();
            }
            AgentEvent::ToolEnd {
                name,
                text,
                is_error,
            } => {
                self.streaming_done();
                let running = self.blocks.iter().rposition(
                    |block| matches!(block, Block::Tool(tool) if tool.state == ToolState::Running),
                );
                match running {
                    Some(index) => {
                        if let Some(Block::Tool(tool)) = self.blocks.get_mut(index) {
                            tool.finish(text, is_error);
                        }
                    }
                    None => {
                        let mut tool =
                            ToolBlock::new(self.blocks.len(), &name, &serde_json::Value::Null);
                        tool.summary = summary_for(&name, &serde_json::Value::Null);
                        tool.finish(text, is_error);
                        self.blocks.push(Block::Tool(tool));
                    }
                }
                self.status = "waiting for model".to_string();
                self.refresh_render();
                self.follow_bottom();
            }
            AgentEvent::Compacted { tokens_before } => {
                if tokens_before > 0 {
                    self.context_tokens = tokens_before;
                }
                self.blocks.push(Block::Compaction { tokens_before });
                self.refresh_render();
                self.follow_bottom();
            }
            AgentEvent::Notice(text) => {
                if let Some(retry) = super::running::retry_text(&text) {
                    self.retry_notice = Some(retry);
                }
                self.blocks
                    .push(Block::Notice(format!("[arqus] {text}"), Level::Info));
                self.status = text;
                self.refresh_render();
                self.follow_bottom();
            }
            AgentEvent::UserMessage(text) => {
                self.dequeue(&text);
                self.push_user(&text);
            }
            AgentEvent::ApprovalRequest { tool, preview } => {
                bell = self.approval_request(tool, preview);
            }
            AgentEvent::Error(text) => {
                self.blocks
                    .push(Block::Notice(format!("error: {text}"), Level::Error));
                self.status = "error".to_string();
                self.retry_notice = None;
                self.refresh_render();
                self.follow_bottom();
            }
            AgentEvent::TurnDone => {
                self.streaming_done();
                self.running = false;
                self.approval = None;
                self.status = "ready".to_string();
                self.retry_notice = None;
                self.refresh_render();
            }
        }
        bell
    }

    fn streaming_done(&mut self) {
        if let Some(Block::Assistant { streaming, .. }) = self.blocks.last_mut() {
            *streaming = false;
        }
    }
}
