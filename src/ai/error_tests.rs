use super::*;

fn body(message: &str) -> String {
    format!("{{\"error\":{{\"message\":{message:?},\"code\":400}}}}")
}

fn config() -> ModelConfig {
    ModelConfig {
        provider: "openai".to_string(),
        model: "m".to_string(),
        base_url: "https://example.test/v1".to_string(),
        api_key: "sk-secret".to_string(),
        context_window: 128_000,
    }
}

#[test]
fn classifies_provider_statuses() {
    assert!(matches!(classify_http(401, None, "{}"), LlmError::Auth(_)));
    assert!(matches!(classify_http(403, None, "{}"), LlmError::Auth(_)));
    assert!(matches!(
        classify_http(402, None, "{}"),
        LlmError::PaymentRequired(_)
    ));
    assert!(matches!(
        classify_http(500, None, "{}"),
        LlmError::Server(500, _)
    ));
    assert!(matches!(
        classify_http(529, None, "{}"),
        LlmError::Server(529, _)
    ));
    assert!(matches!(
        classify_http(404, None, "{}"),
        LlmError::BadRequest(_)
    ));
    assert!(matches!(
        classify_http(200, None, "odd"),
        LlmError::Other(_)
    ));
}

#[test]
fn rate_limit_reads_retry_after() {
    match classify_http(429, Some("7"), "{}") {
        LlmError::RateLimited {
            retry_after_secs, ..
        } => assert_eq!(retry_after_secs, Some(7)),
        other => panic!("expected rate limit, got {other:?}"),
    }
    assert!(matches!(
        classify_http(429, Some("soon"), "{}"),
        LlmError::RateLimited {
            retry_after_secs: None,
            ..
        }
    ));
}

#[test]
fn context_overflow_only_for_known_phrases() {
    for phrase in [
        "context length exceeded",
        "maximum context is 128k",
        "too many tokens",
        "error code context_length_exceeded",
    ] {
        assert!(
            matches!(
                classify_http(400, None, &body(phrase)),
                LlmError::ContextOverflow(_)
            ),
            "phrase: {phrase}"
        );
    }
    assert!(matches!(
        classify_http(413, None, &body("payload too large")),
        LlmError::BadRequest(_)
    ));
    assert!(matches!(
        classify_http(400, None, &body("model not found")),
        LlmError::BadRequest(_)
    ));
}

#[test]
fn error_message_uses_body_text_when_not_json() {
    match classify_http(500, None, "plain failure text") {
        LlmError::Server(500, msg) => assert_eq!(msg, "plain failure text"),
        other => panic!("expected server error, got {other:?}"),
    }
}

#[test]
fn retries_only_transient_errors() {
    assert!(retry_delay(&LlmError::Auth("401".to_string()), 0, 10).is_none());
    assert!(retry_delay(&LlmError::PaymentRequired("402".to_string()), 0, 10).is_none());
    assert!(retry_delay(&LlmError::BadRequest("400".to_string()), 0, 10).is_none());
    assert!(retry_delay(&LlmError::ContextOverflow("x".to_string()), 0, 10).is_none());
    assert!(retry_delay(&LlmError::Server(501, "x".to_string()), 0, 10).is_none());
    assert!(retry_delay(&LlmError::Stream("bad json".to_string()), 0, 10).is_none());
    assert!(retry_delay(&LlmError::Server(503, "x".to_string()), 0, 10).is_some());
    assert!(retry_delay(&LlmError::Network("x".to_string()), 0, 10).is_some());
    assert!(retry_delay(&LlmError::Stream("stream ended early".to_string()), 0, 10).is_some());
}

#[test]
fn retry_delay_honors_cap_and_backoff() {
    let limited = LlmError::RateLimited {
        retry_after_secs: Some(4),
        msg: String::new(),
    };
    assert_eq!(retry_delay(&limited, 0, 10), Some(Duration::from_secs(4)));

    let capped = LlmError::RateLimited {
        retry_after_secs: Some(900),
        msg: String::new(),
    };
    assert_eq!(retry_delay(&capped, 0, 10), Some(Duration::from_secs(60)));

    let server = LlmError::Server(503, "x".to_string());
    let first = retry_delay(&server, 0, 1000).expect("first retry");
    assert!(first.as_millis() >= 1000 && first.as_millis() <= 1250);
    let second = retry_delay(&server, 1, 1000).expect("second retry");
    assert!(second.as_millis() >= 2000 && second.as_millis() <= 2250);
    assert!(retry_delay(&server, MAX_RETRIES, 1000).is_none());
}

#[test]
fn display_is_the_friendly_line() {
    let err = LlmError::Auth("401 Missing key".to_string());
    assert_eq!(
        err.to_string(),
        "Authentication failed (401). Check ARQUS_API_KEY."
    );
    let limited = LlmError::RateLimited {
        retry_after_secs: None,
        msg: "slow down".to_string(),
    };
    assert_eq!(limited.to_string(), "Rate limited (429). slow down");
}

#[test]
fn model_spec_keeps_colons_in_the_model() {
    let (provider, model) =
        crate::ai::split_model_spec("openrouter:deepseek/deepseek-v4-flash:free")
            .expect("colon model");
    assert_eq!(provider, "openrouter");
    assert_eq!(model, "deepseek/deepseek-v4-flash:free");

    let (provider, model) = crate::ai::split_model_spec("a:b:c").expect("split at first colon");
    assert_eq!(provider, "a");
    assert_eq!(model, "b:c");
}

#[test]
fn config_checks_model_and_key() {
    let mut cfg = config();
    assert!(validate_config(&cfg).is_ok());

    cfg.model = "  ".to_string();
    assert!(validate_config(&cfg).unwrap_err().contains("no model"));

    let mut cfg = config();
    cfg.api_key = String::new();
    assert!(validate_config(&cfg).unwrap_err().contains("API key"));

    cfg.base_url = "http://localhost:11434/v1".to_string();
    assert!(validate_config(&cfg).is_ok());
    cfg.base_url = "http://127.0.0.1:1234/v1".to_string();
    assert!(validate_config(&cfg).is_ok());
}

#[test]
fn local_base_url_detection() {
    assert!(is_local_base_url("http://localhost:8080/v1"));
    assert!(is_local_base_url("https://127.0.0.1/v1"));
    assert!(!is_local_base_url("https://localhost.example.com/v1"));
    assert!(!is_local_base_url("https://openrouter.ai/api/v1"));
}
