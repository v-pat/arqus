use super::*;
use crate::types::{Block, StopReason};

fn config() -> ModelConfig {
    ModelConfig {
        provider: "openai".to_string(),
        model: "m".to_string(),
        base_url: "http://x".to_string(),
        api_key: "sk-secret-value".to_string(),
        context_window: 128_000,
    }
}

#[test]
fn debug_redacts_api_key() {
    let text = format!("{:?}", config());
    assert!(!text.contains("sk-secret-value"));
    assert!(text.contains("<redacted>"));
}

fn mock(script: Vec<Vec<StreamEvent>>) -> Llm {
    Llm::Mock(Arc::new(Mutex::new(VecDeque::from(script))))
}

#[tokio::test]
async fn mock_sends_scripted_events_in_order() {
    let done = StreamEvent::Done(Message::Assistant {
        blocks: vec![Block::Text {
            text: "hi".to_string(),
        }],
        provider: "mock".to_string(),
        model: "m".to_string(),
        usage: None,
        stop: StopReason::Stop,
    });
    let first = vec![
        StreamEvent::TextDelta("he".to_string()),
        StreamEvent::TextDelta("llo".to_string()),
        done.clone(),
    ];
    let second = vec![StreamEvent::TextDelta("again".to_string()), done];
    let llm = mock(vec![first.clone(), second.clone()]);

    for expected in [first, second] {
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        llm.stream(
            LlmRequest {
                system: String::new(),
                messages: vec![],
                tools: vec![],
                max_tokens: None,
            },
            tx,
        )
        .await
        .expect("stream");
        let mut got = Vec::new();
        while let Some(ev) = rx.recv().await {
            got.push(ev);
        }
        assert_eq!(got, expected);
    }
}

#[tokio::test]
async fn mock_is_empty_after_script_is_used() {
    let llm = mock(vec![]);
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let err = llm
        .stream(
            LlmRequest {
                system: String::new(),
                messages: vec![],
                tools: vec![],
                max_tokens: None,
            },
            tx,
        )
        .await
        .expect_err("empty mock must fail");
    assert!(err.to_string().contains("no scripted response"));
}

#[test]
fn provider_defaults_differ() {
    assert_eq!(
        default_base_url("anthropic"),
        "https://api.anthropic.com/v1"
    );
    assert_eq!(
        default_base_url("openrouter"),
        "https://openrouter.ai/api/v1"
    );
    assert_eq!(default_base_url("openai"), "https://openrouter.ai/api/v1");
}

#[test]
fn model_spec_splits_provider_and_model() {
    let cfg = ModelConfig::from_model_spec("anthropic:claude-sonnet-4").unwrap();
    assert_eq!(cfg.provider, "anthropic");
    assert_eq!(cfg.model, "claude-sonnet-4");

    let router = ModelConfig::from_model_spec("openrouter:meta-llama/test").unwrap();
    assert_eq!(router.provider, "openrouter");
    assert_eq!(router.model, "meta-llama/test");

    assert!(ModelConfig::from_model_spec("no-colon").is_err());
    assert!(ModelConfig::from_model_spec(":model").is_err());
}

#[test]
fn config_picks_the_right_adapter() {
    assert!(matches!(Llm::from_config(config()), Llm::OpenAi(_)));
    let mut anthropic = config();
    anthropic.provider = "anthropic".to_string();
    assert!(matches!(Llm::from_config(anthropic), Llm::Anthropic(_)));
    let mut router = config();
    router.provider = "openrouter".to_string();
    assert!(matches!(Llm::from_config(router), Llm::OpenRouter(_)));
}
