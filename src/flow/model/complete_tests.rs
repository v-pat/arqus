use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::*;
use crate::ai::{Llm, LlmRequest, StreamEvent};
use crate::types::{Block, Message, StopReason, Usage};

fn make_mock_llm(events: Vec<StreamEvent>) -> Llm {
    Llm::Mock(Arc::new(Mutex::new(VecDeque::from(vec![events]))))
}

fn test_request() -> LlmRequest {
    LlmRequest {
        system: "sys".to_string(),
        messages: vec![Message::User {
            text: "hello".to_string(),
        }],
        tools: vec![],
        max_tokens: Some(100),
    }
}

#[tokio::test]
async fn joins_streamed_text_into_single_string() {
    let events = vec![
        StreamEvent::TextDelta("Hello ".to_string()),
        StreamEvent::TextDelta("world!".to_string()),
        StreamEvent::Done(Message::Assistant {
            blocks: vec![Block::Text {
                text: "Hello world!".to_string(),
            }],
            provider: "mock".to_string(),
            model: "mock-model".to_string(),
            usage: Some(Usage {
                input: 10,
                output: 5,
                cache_read: 0,
                cache_write: 0,
                cost: None,
            }),
            stop: StopReason::Stop,
        }),
    ];
    let llm = make_mock_llm(events);
    let res = complete_text(&llm, test_request())
        .await
        .expect("complete_text should succeed");

    assert_eq!(res.text, "Hello world!");
    assert_eq!(res.usage.input, 10);
    assert_eq!(res.usage.output, 5);
    assert!(!res.estimated);
}

#[tokio::test]
async fn collects_usage_including_cache_read_and_cache_write() {
    let expected_usage = Usage {
        input: 120,
        output: 45,
        cache_read: 80,
        cache_write: 30,
        cost: Some(0.0042),
    };
    let events = vec![
        StreamEvent::TextDelta("answer".to_string()),
        StreamEvent::Done(Message::Assistant {
            blocks: vec![],
            provider: "mock".to_string(),
            model: "m".to_string(),
            usage: Some(expected_usage.clone()),
            stop: StopReason::Stop,
        }),
    ];
    let llm = make_mock_llm(events);
    let res = complete_text(&llm, test_request())
        .await
        .expect("should succeed");

    assert_eq!(res.usage, expected_usage);
    assert_eq!(res.usage.cache_read, 80);
    assert_eq!(res.usage.cache_write, 30);
    assert_eq!(res.usage.cost, Some(0.0042));
    assert!(!res.estimated);
}

#[tokio::test]
async fn provider_error_stream_event_maps_to_provider_error_with_detail() {
    let events = vec![StreamEvent::Error("rate limit exceeded (429)".to_string())];
    let llm = make_mock_llm(events);
    let err = complete_text(&llm, test_request())
        .await
        .expect_err("should return ProviderError");

    match err {
        FlowError::ProviderError(detail) => {
            assert!(
                detail.contains("rate limit exceeded (429)"),
                "detail: {detail}"
            );
        }
        other => panic!("expected ProviderError, got: {other:?}"),
    }
}

#[tokio::test]
async fn provider_stream_failure_maps_to_provider_error_with_detail() {
    // Empty queue causes Mock to return Err
    let llm = Llm::Mock(Arc::new(Mutex::new(VecDeque::new())));
    let err = complete_text(&llm, test_request())
        .await
        .expect_err("should fail when no responses in mock queue");

    match err {
        FlowError::ProviderError(detail) => {
            assert!(
                detail.contains("mock provider") || detail.contains("no scripted response"),
                "detail: {detail}"
            );
        }
        other => panic!("expected ProviderError, got: {other:?}"),
    }
}

#[tokio::test]
async fn fallback_estimates_tokens_when_provider_reports_no_usage() {
    let events = vec![
        StreamEvent::TextDelta("12345678".to_string()), // 8 chars = 2 tokens
        StreamEvent::Done(Message::Assistant {
            blocks: vec![],
            provider: "mock".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::Stop,
        }),
    ];
    let llm = make_mock_llm(events);
    let res = complete_text(&llm, test_request())
        .await
        .expect("should succeed");

    assert_eq!(res.text, "12345678");
    assert!(res.estimated);
    assert_eq!(res.usage.output, 2);
    assert!(res.usage.input > 0);
}

