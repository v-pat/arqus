use std::fmt;
use std::time::Duration;

use serde_json::Value;

use super::ModelConfig;

/// Why a model call failed. Pure data, so retry policy is easy to test.
#[derive(Debug, Clone, PartialEq)]
pub enum LlmError {
    Auth(String),
    PaymentRequired(String),
    RateLimited {
        retry_after_secs: Option<u64>,
        msg: String,
    },
    Server(u16, String),
    ContextOverflow(String),
    BadRequest(String),
    Network(String),
    Stream(String),
    Other(String),
}

/// Hard cap on retries for one request: attempt 0 to 3.
pub const MAX_RETRIES: u32 = 4;

impl LlmError {
    /// One short line for the user. Never contains a key or a request body.
    pub fn friendly(&self) -> String {
        match self {
            LlmError::Auth(msg) => {
                let code = if msg.starts_with("403") { "403" } else { "401" };
                format!("Authentication failed ({code}). Check PIRS_API_KEY.")
            }
            LlmError::PaymentRequired(_) => {
                "No credits left, or this API key's spending limit is used up (402).".to_string()
            }
            LlmError::RateLimited { msg, .. } => {
                if msg.is_empty() {
                    "Rate limited (429).".to_string()
                } else {
                    format!("Rate limited (429). {msg}")
                }
            }
            LlmError::Server(code, msg) => format!("Provider error ({code}). {msg}"),
            LlmError::ContextOverflow(msg) => {
                format!("Context is too large for this model. {msg}")
            }
            LlmError::BadRequest(msg) => format!("Bad request. {msg}"),
            LlmError::Network(msg) => format!("Network error: {msg}"),
            LlmError::Stream(msg) => format!("Stream error: {msg}"),
            LlmError::Other(msg) => msg.clone(),
        }
    }

    /// Short phrase used in a retry notice.
    pub fn label(&self) -> &'static str {
        match self {
            LlmError::RateLimited { .. } => "rate limited",
            LlmError::Server(..) => "provider error",
            LlmError::Network(_) => "network error",
            LlmError::Stream(_) => "stream interrupted",
            _ => "error",
        }
    }

    pub fn is_context_overflow(&self) -> bool {
        matches!(self, LlmError::ContextOverflow(_))
    }
}

/// One friendly line is enough for `anyhow` and for `eprintln!`.
impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.friendly())
    }
}

impl std::error::Error for LlmError {}

/// Classify a non-success HTTP response. Pure, so it is unit tested.
pub fn classify_http(status: u16, retry_after: Option<&str>, body: &str) -> LlmError {
    let msg = error_message(body);
    match status {
        401 | 403 => LlmError::Auth(format!("{status} {msg}")),
        402 => LlmError::PaymentRequired(msg),
        429 => LlmError::RateLimited {
            retry_after_secs: parse_retry_after(retry_after),
            msg,
        },
        500..=599 => LlmError::Server(status, msg),
        400 | 413 if is_context_overflow_text(&msg) => LlmError::ContextOverflow(msg),
        400..=499 => LlmError::BadRequest(msg),
        _ => LlmError::Other(format!("{status} {msg}")),
    }
}

/// `error.message` from a JSON body, else the first 200 characters.
fn error_message(body: &str) -> String {
    let message = serde_json::from_str::<Value>(body).ok().and_then(|value| {
        value
            .get("error")
            .and_then(|err| err.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    match message {
        Some(text) => text,
        None => body.chars().take(200).collect(),
    }
}

fn parse_retry_after(value: Option<&str>) -> Option<u64> {
    value.and_then(|text| text.trim().parse::<u64>().ok())
}

fn is_context_overflow_text(message: &str) -> bool {
    let text = message.to_lowercase();
    [
        "context length",
        "maximum context",
        "too many tokens",
        "context_length_exceeded",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// How long to wait before retry `attempt` (0 based), or `None` to give up.
/// RateLimited and Server delays never exceed 60 s; the rest grow 2x with
/// up to 250 ms of jitter from the clock (no `rand` crate needed).
pub fn retry_delay(err: &LlmError, attempt: u32, base_ms: u64) -> Option<Duration> {
    if attempt >= MAX_RETRIES {
        return None;
    }
    match err {
        LlmError::RateLimited {
            retry_after_secs, ..
        } => {
            if let Some(secs) = retry_after_secs {
                return Some(Duration::from_secs((*secs).min(60)));
            }
        }
        LlmError::Server(code, _) => {
            if !matches!(code, 500 | 502 | 503 | 504 | 529) {
                return None;
            }
        }
        LlmError::Network(_) => {}
        LlmError::Stream(msg) => {
            if !msg.contains("ended early") {
                return None;
            }
        }
        _ => return None,
    }
    let base = base_ms.saturating_mul(1u64 << attempt.min(20));
    Some(Duration::from_millis(base + clock_jitter_ms()))
}

fn clock_jitter_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| u64::from(elapsed.subsec_millis()) % 251)
        .unwrap_or(0)
}

/// Timeouts and retry numbers from the environment. Tests shrink them.
#[derive(Debug, Clone, PartialEq)]
pub struct RetrySettings {
    pub base_ms: u64,
    pub max_retries: u32,
    pub idle: Duration,
}

impl RetrySettings {
    pub fn from_env() -> RetrySettings {
        RetrySettings {
            base_ms: env_number("PIRS_RETRY_BASE_MS", 1000),
            max_retries: env_number("PIRS_MAX_RETRIES", u64::from(MAX_RETRIES)) as u32,
            idle: Duration::from_secs(env_number("PIRS_IDLE_TIMEOUT_SECS", 60)),
        }
    }

    pub fn connect(&self) -> Duration {
        Duration::from_secs(15)
    }
}

fn env_number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(default)
}

/// Text of a failed call for the UI: the friendly line for a known kind of
/// provider error, otherwise the full chain.
pub fn friendly_error(err: &anyhow::Error) -> String {
    match err.downcast_ref::<LlmError>() {
        Some(known) => known.friendly(),
        None => format!("{err:#}"),
    }
}

/// Startup check. A missing key is fine for a local server. Empty model is
/// always a configuration error (exit code 2 in print mode).
pub fn validate_config(cfg: &ModelConfig) -> Result<(), String> {
    if cfg.model.trim().is_empty() {
        return Err(
            "no model configured. Set PIRS_MODEL or use --model provider:model.".to_string(),
        );
    }
    if cfg.api_key.trim().is_empty() && !is_local_base_url(&cfg.base_url) {
        return Err("no API key. Set PIRS_API_KEY (not needed for localhost).".to_string());
    }
    Ok(())
}

/// True for `http://localhost...`, `http://127.0.0.1...` and ports.
pub fn is_local_base_url(base_url: &str) -> bool {
    let rest = base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(base_url);
    let host_port = rest.split('/').next().unwrap_or("");
    let host_port = host_port
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(host_port);
    let host = host_port.split(':').next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1")
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
