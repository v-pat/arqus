use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::StreamEvent;
use crate::types::{Block, Message, StopReason, Usage};

#[derive(Debug, Clone)]
enum BlockBuf {
    Text(String),
    Thinking(String),
    Tool {
        id: String,
        name: String,
        json: String,
    },
}

/// Pure state machine for one Anthropic SSE stream.
#[derive(Debug, Clone)]
pub struct AnthropicParser {
    provider: String,
    model: String,
    open: BTreeMap<usize, BlockBuf>,
    completed: BTreeMap<usize, Block>,
    usage: Usage,
    stop: StopReason,
    finished: bool,
    /// True after `message_stop`.
    pub done: bool,
}

impl AnthropicParser {
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            open: BTreeMap::new(),
            completed: BTreeMap::new(),
            usage: Usage::default(),
            stop: StopReason::Stop,
            finished: false,
            done: false,
        }
    }

    /// Parse one raw SSE line. Lines that are not `data:` are ignored.
    pub fn feed_line(&mut self, line: &str) -> Vec<StreamEvent> {
        let line = line.trim_end_matches(['\r', '\n']);
        let Some(rest) = line.trim_start().strip_prefix("data:") else {
            return Vec::new();
        };
        let data = rest.trim();
        if data.is_empty() {
            return Vec::new();
        }
        if data == "[DONE]" {
            self.done = true;
            return Vec::new();
        }
        self.feed_data(data)
    }

    /// Parse one JSON payload from a `data:` line.
    pub fn feed_data(&mut self, data: &str) -> Vec<StreamEvent> {
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(err) => return vec![StreamEvent::Error(format!("bad SSE JSON: {err}"))],
        };
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();

        match kind {
            "message_start" => {
                if let Some(usage) = value.get("message").and_then(|m| m.get("usage")) {
                    self.read_input_usage(usage);
                }
                Vec::new()
            }
            "content_block_start" => {
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let Some(block) = value.get("content_block") else {
                    return Vec::new();
                };
                let block_type = block
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let buf = match block_type {
                    "text" => BlockBuf::Text(String::new()),
                    "thinking" => BlockBuf::Thinking(String::new()),
                    "tool_use" => BlockBuf::Tool {
                        id: block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        name: block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        json: String::new(),
                    },
                    _ => return Vec::new(),
                };
                self.open.insert(index, buf);
                Vec::new()
            }
            "content_block_delta" => {
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let Some(delta) = value.get("delta") else {
                    return Vec::new();
                };
                let delta_type = delta
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match (delta_type, self.open.get_mut(&index)) {
                    ("text_delta", Some(BlockBuf::Text(text))) => {
                        let piece = delta
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        text.push_str(piece);
                        vec![StreamEvent::TextDelta(piece.to_string())]
                    }
                    ("thinking_delta", Some(BlockBuf::Thinking(text))) => {
                        let piece = delta
                            .get("thinking")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        text.push_str(piece);
                        vec![StreamEvent::ThinkingDelta(piece.to_string())]
                    }
                    ("input_json_delta", Some(BlockBuf::Tool { json, .. })) => {
                        json.push_str(
                            delta
                                .get("partial_json")
                                .and_then(Value::as_str)
                                .unwrap_or_default(),
                        );
                        Vec::new()
                    }
                    _ => Vec::new(),
                }
            }
            "content_block_stop" => {
                let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                self.close_block(index)
            }
            "message_delta" => {
                if let Some(reason) = value
                    .get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(Value::as_str)
                {
                    self.stop = match reason {
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::Length,
                        _ => StopReason::Stop,
                    };
                }
                if let Some(usage) = value.get("usage") {
                    if let Some(n) = usage.get("output_tokens").and_then(Value::as_u64) {
                        self.usage.output = n;
                    }
                }
                Vec::new()
            }
            "message_stop" => {
                self.done = true;
                Vec::new()
            }
            "error" => {
                let err = value.get("error").cloned().unwrap_or(Value::Null);
                vec![StreamEvent::Error(format!("provider error: {err}"))]
            }
            _ => Vec::new(),
        }
    }

    fn read_input_usage(&mut self, usage: &Value) {
        if let Some(n) = usage.get("input_tokens").and_then(Value::as_u64) {
            self.usage.input = n;
        }
        if let Some(n) = usage.get("cache_read_input_tokens").and_then(Value::as_u64) {
            self.usage.cache_read = n;
        }
        if let Some(n) = usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
        {
            self.usage.cache_write = n;
        }
    }

    fn close_block(&mut self, index: usize) -> Vec<StreamEvent> {
        let Some(buf) = self.open.remove(&index) else {
            return Vec::new();
        };
        match buf {
            BlockBuf::Text(text) => {
                if !text.is_empty() {
                    self.completed.insert(index, Block::Text { text });
                }
                Vec::new()
            }
            BlockBuf::Thinking(text) => {
                if !text.is_empty() {
                    self.completed.insert(index, Block::Thinking { text });
                }
                Vec::new()
            }
            BlockBuf::Tool {
                id,
                name,
                json: text,
            } => {
                let args = if text.trim().is_empty() {
                    json!({})
                } else {
                    match serde_json::from_str(&text) {
                        Ok(value) => value,
                        Err(err) => {
                            self.completed.insert(
                                index,
                                Block::ToolCall {
                                    id: id.clone(),
                                    name: name.clone(),
                                    args: Value::Null,
                                },
                            );
                            return vec![StreamEvent::Error(format!(
                                "bad arguments for tool {name}: {err}"
                            ))];
                        }
                    }
                };
                self.completed.insert(
                    index,
                    Block::ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        args: args.clone(),
                    },
                );
                vec![StreamEvent::ToolCall { id, name, args }]
            }
        }
    }

    /// Turn collected blocks into `Done`. Call once, after the stream ends.
    pub fn finish(&mut self) -> Vec<StreamEvent> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;

        let open: Vec<usize> = self.open.keys().copied().collect();
        let mut events = Vec::new();
        for index in open {
            events.extend(self.close_block(index));
        }

        let blocks: Vec<Block> = std::mem::take(&mut self.completed).into_values().collect();
        let usage = if self.usage == Usage::default() {
            None
        } else {
            Some(self.usage.clone())
        };
        events.push(StreamEvent::Done(Message::Assistant {
            blocks,
            provider: self.provider.clone(),
            model: self.model.clone(),
            usage,
            stop: self.stop.clone(),
        }));
        events
    }
}
