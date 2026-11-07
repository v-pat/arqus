use super::*;

fn feed_all(parser: &mut StreamParser, lines: &[&str]) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    for line in lines {
        events.extend(parser.feed_line(line));
    }
    events
}

#[test]
fn text_chunks_become_deltas_then_done() {
    let mut parser = StreamParser::new("openai", "m");
    let events = feed_all(
        &mut parser,
        &[
            r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"lo"},"finish_reason":null}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            "data: [DONE]",
        ],
    );
    assert_eq!(
        events,
        vec![
            StreamEvent::TextDelta("Hel".to_string()),
            StreamEvent::TextDelta("lo".to_string()),
        ]
    );
    assert!(parser.done);

    let tail = parser.finish();
    match &tail[0] {
        StreamEvent::Done(Message::Assistant { blocks, stop, .. }) => {
            assert_eq!(blocks.len(), 1);
            assert_eq!(stop, &StopReason::Stop);
            assert_eq!(
                blocks[0],
                Block::Text {
                    text: "Hello".to_string()
                }
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn newline_terminated_lines_parse() {
    let mut parser = StreamParser::new("openai", "m");
    let events = parser.feed_line("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n");
    assert_eq!(events, vec![StreamEvent::TextDelta("hi".to_string())]);
    assert!(parser.feed_line("data: [DONE]\n").is_empty());
    assert!(parser.done);
}

#[test]
fn tool_call_split_across_three_chunks() {
    let mut parser = StreamParser::new("openai", "m");
    feed_all(
        &mut parser,
        &[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read","arguments":""}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"a.rs\"}"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        ],
    );
    let tail = parser.finish();
    assert_eq!(
        tail[0],
        StreamEvent::ToolCall {
            id: "call_1".to_string(),
            name: "read".to_string(),
            args: json!({"path": "a.rs"}),
        }
    );
    match &tail[1] {
        StreamEvent::Done(Message::Assistant { blocks, stop, .. }) => {
            assert_eq!(stop, &StopReason::ToolUse);
            assert_eq!(blocks.len(), 1);
            assert!(matches!(blocks[0], Block::ToolCall { .. }));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn usage_chunk_sets_input_and_output() {
    let mut parser = StreamParser::new("openai", "m");
    feed_all(
        &mut parser,
        &[
            r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            r#"data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":3}}"#,
        ],
    );
    let tail = parser.finish();
    match &tail[0] {
        StreamEvent::Done(Message::Assistant { usage, .. }) => {
            let usage = usage.as_ref().expect("usage");
            assert_eq!(usage.input, 12);
            assert_eq!(usage.output, 3);
            assert_eq!(usage.cost, None);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn usage_chunk_reads_cost_when_present() {
    let mut parser = StreamParser::new("openrouter", "m");
    feed_all(
        &mut parser,
        &[
            r#"data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":3,"cost":0.0042}}"#,
        ],
    );
    let tail = parser.finish();
    match &tail[0] {
        StreamEvent::Done(Message::Assistant { usage, .. }) => {
            assert_eq!(usage.as_ref().expect("usage").cost, Some(0.0042));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn thinking_uses_reasoning_content_or_reasoning() {
    let mut parser = StreamParser::new("openai", "m");
    let events = feed_all(
        &mut parser,
        &[
            r#"data: {"choices":[{"delta":{"reasoning_content":"think "}}]}"#,
            r#"data: {"choices":[{"delta":{"reasoning":"more"}}]}"#,
        ],
    );
    assert_eq!(
        events,
        vec![
            StreamEvent::ThinkingDelta("think ".to_string()),
            StreamEvent::ThinkingDelta("more".to_string()),
        ]
    );
}

#[test]
fn bad_json_is_an_error_but_bad_arguments_are_kept_for_the_tool() {
    let mut parser = StreamParser::new("openai", "m");
    let events = parser.feed_line("data: not json");
    assert!(matches!(events[0], StreamEvent::Error(_)));

    feed_all(
        &mut parser,
        &[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c","function":{"name":"edit","arguments":"{oops"}}]}}]}"#,
        ],
    );
    let tail = parser.finish();
    // No Error event: the tool turns the raw text into an error result.
    assert_eq!(
        tail[0],
        StreamEvent::ToolCall {
            id: "c".to_string(),
            name: "edit".to_string(),
            args: Value::String("{oops".to_string()),
        }
    );
    assert!(matches!(tail[1], StreamEvent::Done(_)));
}

#[test]
fn cached_tokens_are_not_counted_twice() {
    let mut parser = StreamParser::new("openai", "m");
    feed_all(
        &mut parser,
        &[
            r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            r#"data: {"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":30}}}"#,
        ],
    );
    let tail = parser.finish();
    match &tail[0] {
        StreamEvent::Done(Message::Assistant { usage, .. }) => {
            let usage = usage.as_ref().expect("usage");
            assert_eq!(usage.input, 70);
            assert_eq!(usage.cache_read, 30);
            assert_eq!(usage.output, 20);
            assert_eq!(usage.total(), 120);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn finish_reason_is_tracked_but_done_is_not() {
    let mut parser = StreamParser::new("openai", "m");
    assert!(!parser.saw_finish());
    feed_all(
        &mut parser,
        &[r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#],
    );
    assert!(parser.saw_finish());
    assert!(!parser.done);
}

#[test]
fn provider_error_chunk_keeps_only_the_message() {
    let mut parser = StreamParser::new("openai", "m");
    let events = parser.feed_line(
        r#"data: {"error":{"message":"Provider disconnected","code":502},"choices":[{"index":0,"delta":{"content":""},"finish_reason":"error"}]}"#,
    );
    assert_eq!(
        events,
        vec![StreamEvent::Error(
            "provider error: Provider disconnected (502)".to_string()
        )]
    );
    assert!(parser.saw_finish());
    match &parser.finish()[0] {
        StreamEvent::Done(Message::Assistant { stop, .. }) => {
            assert_eq!(stop, &StopReason::Error)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn non_data_lines_are_ignored() {
    let mut parser = StreamParser::new("openai", "m");
    assert!(parser.feed_line(": keep-alive").is_empty());
    assert!(parser.feed_line("event: message").is_empty());
    assert!(parser.feed_line("").is_empty());
}
