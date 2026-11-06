use super::*;
use crate::types::{Block, Message, StopReason};
use serde_json::json;

fn cfg() -> ModelConfig {
    ModelConfig {
        provider: "openrouter".to_string(),
        model: "meta-llama/test".to_string(),
        base_url: DEFAULT_BASE_URL.to_string(),
        api_key: "test".to_string(),
        context_window: 128_000,
    }
}

#[test]
fn attribution_headers_are_optional_and_named() {
    assert_eq!(
        headers_for(None, None),
        vec![("X-Title", "pirs".to_string())]
    );
    assert_eq!(
        headers_for(Some("https://example.com"), Some("my app")),
        vec![
            ("HTTP-Referer", "https://example.com".to_string()),
            ("X-Title", "my app".to_string()),
        ]
    );
    assert_eq!(
        headers_for(Some("  "), Some(" ")),
        vec![("X-Title", "pirs".to_string())]
    );
}

#[test]
fn body_reuses_the_openai_wire_format() {
    let req = LlmRequest {
        system: "be short".to_string(),
        messages: vec![Message::User {
            text: "hi".to_string(),
        }],
        tools: vec![],
        max_tokens: Some(42),
    };
    let body = build_body(&cfg(), &req);
    assert_eq!(body["model"], "meta-llama/test");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["max_tokens"], 42);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "be short");
    assert_eq!(body["messages"][1]["role"], "user");
}

#[test]
fn tool_call_ids_are_not_rewritten() {
    let messages = vec![
        Message::Assistant {
            blocks: vec![Block::ToolCall {
                id: "call_abc|123".to_string(),
                name: "read".to_string(),
                args: json!({"path": "a.rs"}),
            }],
            provider: "openrouter".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::ToolUse,
        },
        Message::ToolResult {
            call_id: "call_abc|123".to_string(),
            tool_name: "read".to_string(),
            text: "ok".to_string(),
            is_error: false,
        },
    ];
    let converted = super::openai::convert_messages("s", &messages);
    assert_eq!(converted[1]["tool_calls"][0]["id"], "call_abc|123");
    assert_eq!(converted[2]["tool_call_id"], "call_abc|123");
}
