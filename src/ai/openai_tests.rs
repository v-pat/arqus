use super::*;
use crate::types::{Block, Message, StopReason, Usage};
use serde_json::json;

fn cfg() -> ModelConfig {
    ModelConfig {
        provider: "openai".to_string(),
        model: "some-model".to_string(),
        base_url: "http://localhost:1234/v1".to_string(),
        api_key: "test".to_string(),
        context_window: 128_000,
        extra_body: None,
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

#[test]
fn extra_body_merged_shallowly_into_request() {
    let mut config = cfg();
    config.extra_body = Some(json!({
        "temperature": 0.5,
        "chat_template": "custom"
    }));
    let base = LlmRequest {
        system: "sys".to_string(),
        messages: vec![],
        tools: vec![],
        max_tokens: None,
    };

    let body = build_body(&config, &base);
    assert_eq!(body["temperature"], 0.5);
    assert_eq!(body["chat_template"], "custom");
    assert_eq!(body["model"], "some-model");
}

#[test]
fn extra_body_absent_when_empty_or_none() {
    let mut config = cfg();
    let base = LlmRequest {
        system: "sys".to_string(),
        messages: vec![],
        tools: vec![],
        max_tokens: None,
    };

    config.extra_body = None;
    let body_none = build_body(&config, &base);
    assert!(body_none.get("temperature").is_none());

    config.extra_body = Some(json!({}));
    let body_empty = build_body(&config, &base);
    assert!(body_empty.get("temperature").is_none());
}

#[test]
fn extra_body_cannot_override_model_or_messages() {
    let mut config = cfg();
    config.extra_body = Some(json!({
        "model": "malicious-override",
        "messages": [{"role": "user", "content": "injected"}]
    }));
    let base = LlmRequest {
        system: "sys".to_string(),
        messages: vec![Message::User {
            text: "original".to_string(),
        }],
        tools: vec![],
        max_tokens: None,
    };

    let body = build_body(&config, &base);
    assert_eq!(body["model"], "some-model");
    assert_ne!(
        body["messages"],
        json!([{"role": "user", "content": "injected"}])
    );
}
