//! Transcript blocks. Provider-neutral, no ratatui types, no I/O.

use std::time::{Duration, Instant};

use serde_json::Value;
use unicode_width::UnicodeWidthChar;

use crate::types::{Entry, EntryKind, Message};

use super::msg::Level;
use super::styled::display_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolState {
    Running,
    Ok,
    Err,
}

#[derive(Debug, Clone)]
pub struct ToolBlock {
    pub id: usize,
    /// Tool-call id from a session file; empty for a live event.
    pub call_id: String,
    pub name: String,
    pub summary: String,
    pub state: ToolState,
    pub output: String,
    pub started: Instant,
    pub elapsed: Option<Duration>,
    pub args: Value,
}

#[derive(Debug, Clone)]
pub enum Block {
    User(String),
    Assistant { text: String, streaming: bool },
    Thinking { text: String },
    Tool(ToolBlock),
    Notice(String, Level),
    Compaction { tokens_before: u64 },
}

impl Block {
    /// Length of the text that decides whether a cached render is stale.
    pub fn content_len(&self) -> usize {
        match self {
            Block::User(text) => text.len(),
            Block::Assistant { text, .. } => text.len(),
            Block::Thinking { text } => text.len(),
            Block::Tool(tool) => tool.summary.len() + tool.output.len() + tool.name.len(),
            Block::Notice(text, _) => text.len(),
            Block::Compaction { tokens_before } => *tokens_before as usize,
        }
    }
}

impl ToolBlock {
    pub fn new(id: usize, name: &str, args: &Value) -> ToolBlock {
        ToolBlock {
            id,
            call_id: String::new(),
            name: name.to_string(),
            summary: summary_for(name, args),
            state: ToolState::Running,
            output: String::new(),
            started: Instant::now(),
            elapsed: None,
            args: args.clone(),
        }
    }

    pub fn finish(&mut self, text: String, is_error: bool) {
        self.output = cap_lines(&text);
        self.state = if is_error {
            ToolState::Err
        } else {
            ToolState::Ok
        };
        self.elapsed = Some(self.started.elapsed());
    }
}

/// One-line summary shown in a tool card header.
pub fn summary_for(name: &str, args: &Value) -> String {
    let path = args
        .get("path")
        .and_then(|value| value.as_str())
        .unwrap_or("?");
    match name {
        "read" => {
            let offset = args
                .get("offset")
                .and_then(|value| value.as_u64())
                .unwrap_or(1)
                .max(1);
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .unwrap_or(2000)
                .max(1);
            format!("read {path} (lines {offset}-{})", offset + limit - 1)
        }
        "write" => {
            let content = args
                .get("content")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            format!("write {path} ({} lines)", line_count(content))
        }
        "edit" => {
            let old = args
                .get("old_text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let new = args
                .get("new_text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            format!("edit {path} (-{} +{})", line_count(old), line_count(new))
        }
        "bash" => {
            let command = args
                .get("command")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let first = command.lines().next().unwrap_or("").trim();
            format!("bash {}", truncate_width(first, 50))
        }
        other => other.to_string(),
    }
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

/// Longest single line kept in a block. A 100,000 character line would
/// otherwise be wrapped on every frame and freeze the UI.
pub const MAX_LINE_CHARS: usize = 2000;

/// Cut every line to `MAX_LINE_CHARS`, marking the cut with `…`.
pub fn cap_lines(text: &str) -> String {
    if text
        .lines()
        .all(|line| line.chars().count() <= MAX_LINE_CHARS)
    {
        return text.to_string();
    }
    text.split('\n')
        .map(cap_one_line)
        .collect::<Vec<String>>()
        .join("\n")
}

fn cap_one_line(line: &str) -> String {
    if line.chars().count() <= MAX_LINE_CHARS {
        return line.to_string();
    }
    let mut out: String = line.chars().take(MAX_LINE_CHARS).collect();
    out.push('…');
    out
}

/// Cut `text` to `max` display columns, adding `…` when it was cut.
pub fn truncate_width(text: &str, max: usize) -> String {
    if display_width(text) <= max {
        return text.to_string();
    }
    let mut out = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let w = if ch == '\t' {
            4
        } else {
            ch.width().unwrap_or(0)
        };
        if width + w > max.saturating_sub(1) {
            break;
        }
        out.push(ch);
        width += w;
    }
    out.push('…');
    out
}

/// Rebuild the transcript from a loaded session. Tool results fill the output
/// of the tool call that came before them.
pub fn blocks_from_entries(entries: &[Entry]) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for entry in entries {
        match &entry.kind {
            EntryKind::Message { message } => match message {
                Message::User { text } => blocks.push(Block::User(cap_lines(text))),
                Message::Assistant { blocks: parts, .. } => {
                    for part in parts {
                        match part {
                            crate::types::Block::Text { text } => {
                                blocks.push(Block::Assistant {
                                    text: cap_lines(text),
                                    streaming: false,
                                });
                            }
                            crate::types::Block::Thinking { text } => {
                                blocks.push(Block::Thinking {
                                    text: cap_lines(text),
                                });
                            }
                            crate::types::Block::ToolCall { id, name, args } => {
                                let mut tool = ToolBlock::new(blocks.len(), name, args);
                                tool.call_id = id.clone();
                                tool.state = ToolState::Ok;
                                blocks.push(Block::Tool(tool));
                            }
                        }
                    }
                }
                Message::ToolResult {
                    call_id,
                    tool_name,
                    text,
                    is_error,
                } => {
                    let found = blocks.iter_mut().find_map(|block| match block {
                        Block::Tool(tool) if tool.call_id == *call_id => Some(tool),
                        _ => None,
                    });
                    match found {
                        Some(tool) => {
                            tool.output = cap_lines(text);
                            tool.state = if *is_error {
                                ToolState::Err
                            } else {
                                ToolState::Ok
                            };
                        }
                        None => {
                            let mut tool = ToolBlock::new(blocks.len(), tool_name, &Value::Null);
                            tool.call_id = call_id.clone();
                            tool.output = cap_lines(text);
                            tool.state = if *is_error {
                                ToolState::Err
                            } else {
                                ToolState::Ok
                            };
                            blocks.push(Block::Tool(tool));
                        }
                    }
                }
            },
            EntryKind::Compaction { tokens_before, .. } => blocks.push(Block::Compaction {
                tokens_before: *tokens_before,
            }),
            EntryKind::BranchSummary { summary, .. } => {
                blocks.push(Block::Notice(summary.clone(), Level::Info));
            }
            EntryKind::ModelChange { .. } | EntryKind::Custom { .. } => {}
        }
    }
    blocks
}

#[cfg(test)]
#[path = "block_tests.rs"]
mod tests;
