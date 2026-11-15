use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

use super::sse::StreamParser;
use super::{
    classify_http, LlmError, LlmRequest, ModelConfig, RetrySettings, StreamEvent, ToolDef,
};
use crate::types::{Block, Message};

/// Convert neutral messages to the OpenAI chat/completions wire format.
pub fn convert_messages(system: &str, messages: &[Message]) -> Vec<Value> {
    let mut out = vec![json!({"role": "system", "content": system})];

    for message in messages {
        match message {
            Message::User { text } => out.push(json!({"role": "user", "content": text})),
            Message::Assistant { blocks, .. } => {
                let mut content = String::new();
                let mut thinking = String::new();
                let mut calls = Vec::new();
                for block in blocks {
                    match block {
                        Block::Text { text } => content.push_str(text),
                        Block::Thinking { text } => thinking.push_str(text),
                        Block::ToolCall { id, name, args } => calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {"name": name, "arguments": args.to_string()},
                        })),
                    }
                }
                let mut value = json!({"role": "assistant", "content": content});
                if !calls.is_empty() {
                    value["tool_calls"] = Value::Array(calls);
                }
                // Quirk: reasoning models reject a follow-up turn that drops
                // the thinking text. Keep it inside this adapter only.
                if !thinking.is_empty() {
                    value["reasoning_content"] = json!(thinking);
                }
                out.push(value);
            }
            Message::ToolResult { call_id, text, .. } => out.push(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": text,
            })),
        }
    }
    out
}

fn tool_json(tool: &ToolDef) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        },
    })
}

/// Build the POST body. `tools` is omitted when empty.
pub fn build_body(cfg: &ModelConfig, req: &LlmRequest) -> Value {
    let mut body = json!({
        "model": cfg.model,
        "messages": convert_messages(&req.system, &req.messages),
        "stream": true,
        "stream_options": {"include_usage": true},
    });
    if !req.tools.is_empty() {
        body["tools"] = Value::Array(req.tools.iter().map(tool_json).collect());
    }
    if let Some(max_tokens) = req.max_tokens {
        body["max_tokens"] = json!(max_tokens);
    }
    if let Some(Value::Object(map)) = &cfg.extra_body {
        for (k, v) in map {
            if k != "model" && k != "messages" {
                body[k] = v.clone();
            }
        }
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
    stream_with_headers(cfg, req, tx, &[]).await
}

/// Same call with extra request headers. OpenRouter uses this hook.
pub async fn stream_with_headers(
    cfg: &ModelConfig,
    req: LlmRequest,
    tx: Sender<StreamEvent>,
    headers: &[(&str, String)],
) -> std::result::Result<(), LlmError> {
    let settings = RetrySettings::from_env();
    let idle = settings.idle;
    let idle_text = format!("no data for {}s", idle.as_secs());
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = build_body(cfg, &req);
    crate::debug::log("request", json!({"url": url, "body": body}));
    let client = reqwest::Client::builder()
        .connect_timeout(settings.connect())
        .build()
        .map_err(|err| LlmError::Other(format!("cannot build HTTP client: {err}")))?;
    let mut request = client.post(&url).bearer_auth(&cfg.api_key).json(&body);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let response = tokio::time::timeout(idle, request.send())
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

    let mut parser = StreamParser::new(&cfg.provider, &cfg.model);
    let mut bytes = response.bytes_stream();
    // Buffer raw BYTES, not text: a network chunk can cut a line or a UTF-8
    // character in half. Decode only complete lines.
    let mut buffer: Vec<u8> = Vec::new();

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
        buffer.extend_from_slice(&chunk);
        while let Some(position) = buffer.iter().position(|b| *b == b'\n') {
            let line = String::from_utf8_lossy(&buffer.drain(..=position).collect::<Vec<u8>>())
                .to_string();
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

    // A stream may end without a trailing newline.
    if !parser.done && buffer.iter().any(|b| !b.is_ascii_whitespace()) {
        let line = String::from_utf8_lossy(&buffer).to_string();
        crate::debug::log("sse", json!({"line": line}));
        for event in parser.feed_line(&line) {
            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }
    }

    // No finish_reason and no [DONE]: the server closed the stream mid-answer.
    if !parser.done && !parser.saw_finish() {
        let text = "stream ended early: no finish_reason and no [DONE]".to_string();
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
#[path = "openai_tests.rs"]
mod tests;
