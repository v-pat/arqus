use super::*;
use crate::types::{StopReason, Usage};
use serde_json::json;

fn cfg() -> ModelConfig {
    ModelConfig {
        provider: "anthropic".to_string(),
        model: "claude-test".to_string(),
        base_url: "https://api.anthropic.com/v1".to_string(),
        api_key: "test".to_string(),
        context_window: 200_000,
    }
}

fn request() -> LlmRequest {
    LlmRequest {
        system: "be short".to_string(),
        messages: vec![],
        tools: vec![],
        max_tokens: None,
    }
}

fn call(id: &str, name: &str) -> Block {
    Block::ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args: json!({"path": "a.rs"}),
    }
}

fn result(id: &str, text: &str) -> Message {
    Message::ToolResult {
        call_id: id.to_string(),
        tool_name: "read".to_string(),
        text: text.to_string(),
        is_error: false,
    }
}

#[test]
fn converts_roles_and_merges_consecutive_tool_results() {
    let messages = vec![
        Message::User {
            text: "go".to_string(),
        },
        Message::Assistant {
            blocks: vec![
                Block::Text {
                    text: "Checking.".to_string(),
                },
                Block::Thinking {
                    text: "hidden".to_string(),
                },
                call("call_abc|123", "read"),
                call("call_2", "bash"),
            ],
            provider: "openai".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::ToolUse,
        },
        result("call_abc|123", "a"),
        result("call_2", "b"),
        Message::User {
            text: "thanks".to_string(),
        },
    ];

    let got = Value::Array(convert_messages(&messages));
    let expected = json!([
        {"role": "user", "content": "go"},
        {
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Checking."},
                {"type": "tool_use", "id": "call_abc_123", "name": "read", "input": {"path": "a.rs"}},
                {"type": "tool_use", "id": "call_2", "name": "bash", "input": {"path": "a.rs"}}
            ]
        },
        {
            "role": "user",
            "content": [
                {"type": "tool_result", "tool_use_id": "call_abc_123", "content": "a", "is_error": false},
                {"type": "tool_result", "tool_use_id": "call_2", "content": "b", "is_error": false},
                {"type": "text", "text": "thanks"}
            ]
        }
    ]);
    assert_eq!(got, expected);
}

#[test]
fn merges_tool_result_then_user_text_into_one_user_turn() {
    let messages = vec![
        Message::Assistant {
            blocks: vec![call("call_1", "read")],
            provider: "openai".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::ToolUse,
        },
        result("call_1", "body"),
        Message::User {
            text: "now fix it".to_string(),
        },
    ];

    let got = Value::Array(convert_messages(&messages));
    let expected = json!([
        {
            "role": "assistant",
            "content": [
                {"type": "tool_use", "id": "call_1", "name": "read", "input": {"path": "a.rs"}}
            ]
        },
        {
            "role": "user",
            "content": [
                {"type": "tool_result", "tool_use_id": "call_1", "content": "body", "is_error": false},
                {"type": "text", "text": "now fix it"}
            ]
        }
    ]);
    assert_eq!(got, expected);
}

#[test]
fn sanitizes_foreign_ids_and_truncates_to_64() {
    assert_eq!(sanitize_tool_id("call_abc|123"), "call_abc_123");
    assert_eq!(sanitize_tool_id(""), "tool_call");
    let long = "a".repeat(70);
    assert_eq!(sanitize_tool_id(&long).chars().count(), 64);
    assert!(sanitize_tool_id("a b/c")
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));
}

#[test]
fn skips_thinking_only_assistant_messages() {
    let messages = vec![Message::Assistant {
        blocks: vec![Block::Thinking {
            text: "only thought".to_string(),
        }],
        provider: "openai".to_string(),
        model: "m".to_string(),
        usage: None,
        stop: StopReason::Stop,
    }];
    assert!(convert_messages(&messages).is_empty());
}

#[test]
fn body_has_required_max_tokens_and_tool_schema() {
    let plain = build_body(&cfg(), &request());
    assert_eq!(plain["model"], "claude-test");
    assert_eq!(plain["max_tokens"], 8192);
    assert_eq!(plain["system"], "be short");
    assert_eq!(plain["stream"], true);
    assert!(plain.get("tools").is_none());

    let full = build_body(
        &cfg(),
        &LlmRequest {
            tools: vec![ToolDef {
                name: "read".to_string(),
                description: "read a file".to_string(),
                parameters: json!({"type": "object"}),
            }],
            max_tokens: Some(99),
            ..request()
        },
    );
    assert_eq!(full["max_tokens"], 99);
    assert_eq!(
        full["tools"],
        json!([{
            "name": "read",
            "description": "read a file",
            "input_schema": {"type": "object"}
        }])
    );
}

fn data(parser: &mut AnthropicParser, value: Value) -> Vec<StreamEvent> {
    parser.feed_data(&value.to_string())
}

#[test]
fn parser_collects_text_thinking_and_tool_json() {
    let mut parser = AnthropicParser::new("anthropic", "claude-test");
    assert!(data(
        &mut parser,
        json!({
            "type": "message_start",
            "message": {"usage": {"input_tokens": 12, "cache_read_input_tokens": 3}}
        })
    )
    .is_empty());

    // text block
    data(
        &mut parser,
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text"}}),
    );
    let text = data(
        &mut parser,
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "hello"}}),
    );
    assert_eq!(text, vec![StreamEvent::TextDelta("hello".to_string())]);
    data(
        &mut parser,
        json!({"type": "content_block_stop", "index": 0}),
    );

    // thinking block
    data(
        &mut parser,
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "thinking"}}),
    );
    let thinking = data(
        &mut parser,
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "thinking_delta", "thinking": "why"}}),
    );
    assert_eq!(
        thinking,
        vec![StreamEvent::ThinkingDelta("why".to_string())]
    );
    data(
        &mut parser,
        json!({"type": "content_block_stop", "index": 1}),
    );

    // tool block with split JSON
    data(
        &mut parser,
        json!({"type": "content_block_start", "index": 2, "content_block": {"type": "tool_use", "id": "call_abc|123", "name": "read"}}),
    );
    for piece in ["{\"path\":", "\"a.rs\"}"] {
        data(
            &mut parser,
            json!({"type": "content_block_delta", "index": 2, "delta": {"type": "input_json_delta", "partial_json": piece}}),
        );
    }
    let tool = data(
        &mut parser,
        json!({"type": "content_block_stop", "index": 2}),
    );
    assert_eq!(
        tool,
        vec![StreamEvent::ToolCall {
            id: "call_abc|123".to_string(),
            name: "read".to_string(),
            args: json!({"path": "a.rs"}),
        }]
    );

    data(
        &mut parser,
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 5}}),
    );
    assert!(data(&mut parser, json!({"type": "message_stop"})).is_empty());
    assert!(parser.done);

    let done = parser.finish();
    match done.as_slice() {
        [StreamEvent::Done(Message::Assistant {
            blocks,
            provider,
            model,
            usage,
            stop,
        })] => {
            assert_eq!(provider, "anthropic");
            assert_eq!(model, "claude-test");
            assert_eq!(*stop, StopReason::ToolUse);
            assert_eq!(
                usage,
                &Some(Usage {
                    input: 12,
                    output: 5,
                    cache_read: 3,
                    cache_write: 0,
                    cost: None,
                })
            );
            assert_eq!(
                blocks.as_slice(),
                [
                    Block::Text {
                        text: "hello".to_string()
                    },
                    Block::Thinking {
                        text: "why".to_string()
                    },
                    Block::ToolCall {
                        id: "call_abc|123".to_string(),
                        name: "read".to_string(),
                        args: json!({"path": "a.rs"}),
                    }
                ]
            );
        }
        other => panic!("expected one Done, got {other:?}"),
    }
}

#[test]
fn stop_reasons_map_end_turn_and_max_tokens() {
    for (raw, expected) in [
        ("end_turn", StopReason::Stop),
        ("max_tokens", StopReason::Length),
        ("tool_use", StopReason::ToolUse),
    ] {
        let mut parser = AnthropicParser::new("anthropic", "m");
        data(
            &mut parser,
            json!({"type": "message_delta", "delta": {"stop_reason": raw}}),
        );
        let events = parser.finish();
        match events.last() {
            Some(StreamEvent::Done(Message::Assistant { stop, .. })) => {
                assert_eq!(*stop, expected, "raw {raw}");
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }
}

#[test]
fn close_without_stop_event_still_parses_tool_json() {
    let mut parser = AnthropicParser::new("anthropic", "m");
    data(
        &mut parser,
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "c1", "name": "bash"}}),
    );
    data(
        &mut parser,
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{}"}}),
    );
    let events = parser.finish();
    assert!(matches!(events[0], StreamEvent::ToolCall { .. }));
    assert!(matches!(events[1], StreamEvent::Done(_)));
}
