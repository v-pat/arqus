use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

use super::{
    classify_http, LlmError, LlmRequest, ModelConfig, RetrySettings, StreamEvent, ToolDef,
};
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
                // Quirk: a queued user text right after the results must join
                // the same user turn. Anthropic wants one turn here, and the
                // tool results come first.
                while let Some(Message::User { text }) = messages.get(i) {
                    if !text.is_empty() {
                        content.push(text_block(text));
                    }
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
pub async fn stream(
    cfg: &ModelConfig,
    req: LlmRequest,
    tx: Sender<StreamEvent>,
) -> std::result::Result<(), LlmError> {
    let settings = RetrySettings::from_env();
    let idle = settings.idle;
    let idle_text = format!("no data for {}s", idle.as_secs());
    let url = format!("{}/messages", cfg.base_url.trim_end_matches('/'));
    let body = build_body(cfg, &req);
    crate::debug::log("request", json!({"url": url, "body": body}));
    let client = reqwest::Client::builder()
        .connect_timeout(settings.connect())
        .build()
        .map_err(|err| LlmError::Other(format!("cannot build HTTP client: {err}")))?;
    let response = tokio::time::timeout(
        idle,
        client
            .post(&url)
            .header("x-api-key", &cfg.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send(),
    )
    .await
    .map_err(|_| LlmError::Network(idle_text.clone()))?
    .map_err(|err| LlmError::Network(format!("POST {url} failed: {err}")))?;

    let status = response.status();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    crate::debug::log("status", json!({"code": status.as_u16()}));
    if !status.is_success() {
        let body = tokio::time::timeout(idle, response.text())
            .await
            .ok()
            .and_then(|text| text.ok())
            .unwrap_or_default();
        crate::debug::log("error", json!({"text": short(&body)}));
        return Err(classify_http(
            status.as_u16(),
            retry_after.as_deref(),
            &body,
        ));
    }

    let mut parser = AnthropicParser::new(&cfg.provider, &cfg.model);
    let mut bytes = response.bytes_stream();
    let mut buffer = String::new();

    'read: loop {
        let next = tokio::time::timeout(idle, bytes.next()).await;
        let chunk = match next {
            Err(_) => return Err(LlmError::Network(idle_text.clone())),
            Ok(None) => break,
            Ok(Some(Err(err))) => {
                return Err(LlmError::Network(format!("reading response stream: {err}")))
            }
            Ok(Some(Ok(chunk))) => chunk,
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(position) = buffer.find('\n') {
            let line: String = buffer.drain(..=position).collect();
            crate::debug::log("sse", json!({"line": line}));
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
        crate::debug::log("sse", json!({"line": buffer}));
        for event in parser.feed_line(&buffer) {
            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }
    }

    // No message_stop: the server closed the stream mid-answer.
    if !parser.done {
        let text = "stream ended early: no message_stop".to_string();
        crate::debug::log("error", json!({"text": text}));
        return Err(LlmError::Stream(text));
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
