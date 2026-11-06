use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

use super::{LlmRequest, ModelConfig, StreamEvent, ToolDef};
use crate::types::{Block, Message};

#[path = "anthropic_sse.rs"]
mod anthropic_sse;
pub use anthropic_sse::AnthropicParser;

/// Anthropic tool ids may only contain letters, digits, `_` and `-`, and are
/// at most 64 characters. Neutral ids come from any provider, so map them here.
///
/// Rust note: `map` on a `char` iterator applies one function to every item.
/// `take` stops after 64 items, and `collect` builds the `String` back.
pub fn sanitize_tool_id(id: &str) -> String {
    let clean: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if clean.is_empty() {
        "tool_call".to_string()
    } else {
        clean
    }
}

fn text_block(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

fn tool_result_block(call_id: &str, text: &str, is_error: bool) -> Value {
    json!({
        "type": "tool_result",
        "tool_use_id": sanitize_tool_id(call_id),
        "content": text,
        "is_error": is_error,
    })
}

/// Convert neutral messages to the Anthropic messages array.
/// Consecutive tool results become ONE user message.
pub fn convert_messages(messages: &[Message]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        match &messages[i] {
            Message::User { text } => {
                out.push(json!({"role": "user", "content": text}));
                i += 1;
            }
            Message::Assistant { blocks, .. } => {
                let mut content = Vec::new();
                for block in blocks {
                    match block {
                        Block::Text { text } => {
                            if !text.is_empty() {
                                content.push(text_block(text));
                            }
                        }
                        // Quirk: Anthropic has no neutral thinking block here.
                        Block::Thinking { .. } => {}
                        Block::ToolCall { id, name, args } => content.push(json!({
                            "type": "tool_use",
                            "id": sanitize_tool_id(id),
                            "name": name,
                            "input": args,
                        })),
                    }
                }
                // An empty assistant message is not legal, and only a
                // thinking block produces one. Skip it.
                if !content.is_empty() {
                    out.push(json!({"role": "assistant", "content": content}));
                }
                i += 1;
            }
            Message::ToolResult { .. } => {
                let mut content = Vec::new();
                while let Some(Message::ToolResult {
                    call_id,
                    text,
                    is_error,
                    ..
                }) = messages.get(i)
                {
                    content.push(tool_result_block(call_id, text, *is_error));
                    i += 1;
                }
                out.push(json!({"role": "user", "content": content}));
            }
        }
    }

    out
}

fn tool_json(tool: &ToolDef) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.parameters,
    })
}

/// `max_tokens` is required by Anthropic, so use the default when unset.
pub fn build_body(cfg: &ModelConfig, req: &LlmRequest) -> Value {
    let mut body = json!({
        "model": cfg.model,
        "max_tokens": req.max_tokens.unwrap_or(8192),
        "system": req.system,
        "messages": convert_messages(&req.messages),
        "stream": true,
    });
    if !req.tools.is_empty() {
        body["tools"] = Value::Array(req.tools.iter().map(tool_json).collect());
    }
    body
}

fn short(text: &str) -> String {
    text.chars().take(500).collect()
}

/// POST the request and forward parsed stream events to `tx`.
pub async fn stream(cfg: &ModelConfig, req: LlmRequest, tx: Sender<StreamEvent>) -> Result<()> {
    let url = format!("{}/messages", cfg.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&build_body(cfg, &req))
        .send()
        .await
        .with_context(|| format!("POST {url} failed"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("provider returned {status}: {}", short(&body));
    }

    let mut parser = AnthropicParser::new(&cfg.provider, &cfg.model);
    let mut bytes = response.bytes_stream();
    let mut buffer = String::new();

    'read: while let Some(chunk) = bytes.next().await {
        let chunk = chunk.context("reading response stream")?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(position) = buffer.find('\n') {
            let line: String = buffer.drain(..=position).collect();
            for event in parser.feed_line(&line) {
                if tx.send(event).await.is_err() {
                    return Ok(());
                }
            }
            if parser.done {
                break 'read;
            }
        }
    }

    if !parser.done && !buffer.trim().is_empty() {
        for event in parser.feed_line(&buffer) {
            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }
    }

    for event in parser.finish() {
        if tx.send(event).await.is_err() {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "anthropic_tests.rs"]
mod tests;
