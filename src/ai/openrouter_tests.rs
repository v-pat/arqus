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
        vec![("X-Title", "arqus".to_string())]
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
        vec![("X-Title", "arqus".to_string())]
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
fn budget_line_shows_the_three_numbers() {
    let value = json!({"data":{"limit":20.0,"limit_remaining":12.5,"usage_daily":0.25}});
    let line = budget_line(&value);
    assert_eq!(
        line,
        "limit_remaining=$12.5000 limit=$20.0000 usage_daily=$0.2500"
    );
    assert!(line.contains("limit_remaining"));
}

#[test]
fn budget_line_says_no_limit_when_remaining_is_null() {
    let value = json!({"data":{"limit":null,"limit_remaining":null,"usage_daily":0.0}});
    let line = budget_line(&value);
    assert!(line.contains("limit_remaining=no limit"), "{line}");
    assert!(line.contains("limit=none"), "{line}");
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
