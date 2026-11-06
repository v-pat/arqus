use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

use super::sse::StreamParser;
use super::{LlmRequest, ModelConfig, StreamEvent, ToolDef};
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
    body
}

fn short(text: &str) -> String {
    text.chars().take(500).collect()
}

/// POST the request and forward parsed stream events to `tx`.
pub async fn stream(cfg: &ModelConfig, req: LlmRequest, tx: Sender<StreamEvent>) -> Result<()> {
    stream_with_headers(cfg, req, tx, &[]).await
}

/// Same call with extra request headers. OpenRouter uses this hook.
pub async fn stream_with_headers(
    cfg: &ModelConfig,
    req: LlmRequest,
    tx: Sender<StreamEvent>,
    headers: &[(&str, String)],
) -> Result<()> {
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut request = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&build_body(cfg, &req));
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let response = request
        .send()
        .await
        .with_context(|| format!("POST {url} failed"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("provider returned {status}: {}", short(&body));
    }

    let mut parser = StreamParser::new(&cfg.provider, &cfg.model);
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

    // A stream may end without a trailing newline.
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
mod tests {
    use super::*;
    use crate::types::{StopReason, Usage};

    fn cfg() -> ModelConfig {
        ModelConfig {
            provider: "openai".to_string(),
            model: "some-model".to_string(),
            base_url: "http://localhost:1234/v1".to_string(),
            api_key: "test".to_string(),
            context_window: 128_000,
        }
    }

    #[test]
    fn converts_every_message_kind() {
        let messages = vec![
            Message::User {
                text: "list the files".to_string(),
            },
            Message::Assistant {
                blocks: vec![
                    Block::Text {
                        text: "Checking.".to_string(),
                    },
                    Block::Thinking {
                        text: "think".to_string(),
                    },
                    Block::ToolCall {
                        id: "call_1".to_string(),
                        name: "bash".to_string(),
                        args: json!({"command": "ls"}),
                    },
                ],
                provider: "openai".to_string(),
                model: "m".to_string(),
                usage: Some(Usage::default()),
                stop: StopReason::ToolUse,
            },
            Message::ToolResult {
                call_id: "call_1".to_string(),
                tool_name: "bash".to_string(),
                text: "Cargo.toml\nsrc".to_string(),
                is_error: false,
            },
        ];
        let got = Value::Array(convert_messages("be short", &messages));
        let expected = json!([
            {"role": "system", "content": "be short"},
            {"role": "user", "content": "list the files"},
            {
                "role": "assistant",
                "content": "Checking.",
                "reasoning_content": "think",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "bash", "arguments": "{\"command\":\"ls\"}"}
                }]
            },
            {"role": "tool", "tool_call_id": "call_1", "content": "Cargo.toml\nsrc"},
        ]);
        assert_eq!(got, expected);
    }

    #[test]
    fn assistant_without_tool_calls_has_no_tool_calls_field() {
        let messages = vec![Message::Assistant {
            blocks: vec![Block::Text {
                text: "done".to_string(),
            }],
            provider: "openai".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::Stop,
        }];
        let got = convert_messages("s", &messages);
        assert_eq!(got[1], json!({"role": "assistant", "content": "done"}));
    }

    #[test]
    fn body_has_tools_only_when_present() {
        let tool = ToolDef {
            name: "read".to_string(),
            description: "read a file".to_string(),
            parameters: json!({"type": "object"}),
        };
        let base = LlmRequest {
            system: "s".to_string(),
            messages: vec![],
            tools: vec![],
            max_tokens: None,
        };

        let plain = build_body(&cfg(), &base);
        assert_eq!(plain["model"], "some-model");
        assert_eq!(plain["stream"], true);
        assert_eq!(plain["stream_options"]["include_usage"], true);
        assert!(plain.get("tools").is_none());
        assert!(plain.get("max_tokens").is_none());

        let full = build_body(
            &cfg(),
            &LlmRequest {
                tools: vec![tool],
                max_tokens: Some(99),
                ..base
            },
        );
        assert_eq!(
            full["tools"],
            json!([{
                "type": "function",
                "function": {
                    "name": "read",
                    "description": "read a file",
                    "parameters": {"type": "object"}
                }
            }])
        );
        assert_eq!(full["max_tokens"], 99);
    }
}
