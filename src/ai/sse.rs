use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::StreamEvent;
use crate::types::{Block, Message, StopReason, Usage};

#[derive(Debug, Clone, Default)]
struct ToolFragment {
    id: String,
    name: String,
    args: String,
}

/// Pure state machine for one SSE stream. Give it lines, get events back.
/// No network, no async, so it is easy to unit test.
#[derive(Debug, Clone)]
pub struct StreamParser {
    provider: String,
    model: String,
    text: String,
    thinking: String,
    tools: BTreeMap<usize, ToolFragment>,
    usage: Usage,
    stop: StopReason,
    finished: bool,
    /// True once a chunk carried a `finish_reason`.
    saw_finish: bool,
    /// True after `data: [DONE]`.
    pub done: bool,
}

impl StreamParser {
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            text: String::new(),
            thinking: String::new(),
            tools: BTreeMap::new(),
            usage: Usage::default(),
            stop: StopReason::Stop,
            finished: false,
            saw_finish: false,
            done: false,
        }
    }

    /// True once the stream reported a `finish_reason`. A stream that ends
    /// after it is complete even without `data: [DONE]`.
    pub fn saw_finish(&self) -> bool {
        self.saw_finish
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

        if let Some(err) = value.get("error") {
            // An error chunk can repeat finish_reason. Treat it as the end of
            // the stream so a following close is not reported twice.
            self.saw_finish = true;
            self.stop = StopReason::Error;
            return vec![StreamEvent::Error(provider_error_text(err))];
        }

        if let Some(usage) = value.get("usage").filter(|u| !u.is_null()) {
            // OpenRouter and OpenAI include cached tokens inside
            // `prompt_tokens`; subtract them so `Usage::total()` stays right.
            let cached = usage
                .get("prompt_tokens_details")
                .and_then(|details| details.get("cached_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if let Some(n) = usage.get("prompt_tokens").and_then(Value::as_u64) {
                self.usage.input = n.saturating_sub(cached);
            }
            self.usage.cache_read = cached;
            if let Some(n) = usage.get("completion_tokens").and_then(Value::as_u64) {
                self.usage.output = n;
            }
            // OpenRouter reports the request cost in credits (roughly dollars).
            if let Some(cost) = usage.get("cost").and_then(Value::as_f64) {
                self.usage.cost = Some(cost);
            }
        }

        let Some(choice) = value.get("choices").and_then(|c| c.get(0)) else {
            return Vec::new();
        };
        let mut events = Vec::new();

        if let Some(delta) = choice.get("delta") {
            if let Some(text) = delta.get("content").and_then(Value::as_str) {
                if !text.is_empty() {
                    self.text.push_str(text);
                    events.push(StreamEvent::TextDelta(text.to_string()));
                }
            }
            let thinking = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .and_then(Value::as_str);
            if let Some(text) = thinking {
                if !text.is_empty() {
                    self.thinking.push_str(text);
                    events.push(StreamEvent::ThinkingDelta(text.to_string()));
                }
            }
            if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for call in calls {
                    let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    let fragment = self.tools.entry(index).or_default();
                    if let Some(id) = call.get("id").and_then(Value::as_str) {
                        fragment.id.push_str(id);
                    }
                    if let Some(function) = call.get("function") {
                        if let Some(name) = function.get("name").and_then(Value::as_str) {
                            fragment.name.push_str(name);
                        }
                        if let Some(args) = function.get("arguments").and_then(Value::as_str) {
                            fragment.args.push_str(args);
                        }
                    }
                }
            }
        }

        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.saw_finish = true;
            self.stop = match reason {
                "tool_calls" => StopReason::ToolUse,
                "length" => StopReason::Length,
                "error" => StopReason::Error,
                _ => StopReason::Stop,
            };
        }
        events
    }

    /// Turn collected fragments into complete `ToolCall` events plus `Done`.
    /// Call once, after the stream ends, so the usage chunk is included.
    pub fn finish(&mut self) -> Vec<StreamEvent> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut events = Vec::new();
        let mut blocks = Vec::new();

        if !self.text.is_empty() {
            blocks.push(Block::Text {
                text: self.text.clone(),
            });
        }
        if !self.thinking.is_empty() {
            blocks.push(Block::Thinking {
                text: self.thinking.clone(),
            });
        }

        for (_, fragment) in std::mem::take(&mut self.tools) {
            if fragment.id.is_empty() && fragment.name.is_empty() {
                continue;
            }
            let args = if fragment.args.trim().is_empty() {
                json!({})
            } else {
                match serde_json::from_str(&fragment.args) {
                    Ok(value) => value,
                    // Keep the raw text. `tools::execute` turns it into an
                    // error result, so the loop can continue (A15).
                    Err(_) => Value::String(fragment.args.clone()),
                }
            };
            events.push(StreamEvent::ToolCall {
                id: fragment.id.clone(),
                name: fragment.name.clone(),
                args: args.clone(),
            });
            blocks.push(Block::ToolCall {
                id: fragment.id,
                name: fragment.name,
                args,
            });
        }

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

/// One readable line for a provider error object:
/// `provider error: Provider disconnected (502)`.
fn provider_error_text(err: &Value) -> String {
    let message = err
        .get("message")
        .and_then(Value::as_str)
        .map(|text| text.to_string())
        .unwrap_or_else(|| err.to_string());
    let code = match err.get("code") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(value) if !value.is_null() => Some(value.to_string()),
        _ => None,
    };
    match code {
        Some(code) => format!("provider error: {message} ({code})"),
        None => format!("provider error: {message}"),
    }
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
