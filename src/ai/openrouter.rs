//! OpenRouter adapter. Its wire format is OpenAI-compatible, so conversion
//! and SSE parsing stay in `openai.rs`. This module owns the provider name,
//! the default endpoint, and the optional attribution headers.

use serde_json::Value;
use tokio::sync::mpsc::Sender;

use super::openai;
use super::{LlmError, LlmRequest, ModelConfig, RetrySettings, StreamEvent};

pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const APP_TITLE: &str = "pirs";

/// OpenRouter uses `HTTP-Referer` and `X-Title` for app attribution.
/// Both are optional; the title has a default so the app is identifiable.
pub fn headers_for(referer: Option<&str>, title: Option<&str>) -> Vec<(&'static str, String)> {
    let mut headers = Vec::new();
    if let Some(referer) = referer.map(str::trim).filter(|text| !text.is_empty()) {
        headers.push(("HTTP-Referer", referer.to_string()));
    }
    let title = title
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(APP_TITLE);
    headers.push(("X-Title", title.to_string()));
    headers
}

/// Headers from the environment. `PIRS_HTTP_REFERER` is optional.
pub fn extra_headers() -> Vec<(&'static str, String)> {
    let referer = std::env::var("PIRS_HTTP_REFERER").ok();
    let title = std::env::var("PIRS_APP_TITLE").ok();
    headers_for(referer.as_deref(), title.as_deref())
}

/// Kept here so a test can check that OpenRouter reuses the OpenAI body.
pub fn build_body(cfg: &ModelConfig, req: &LlmRequest) -> Value {
    openai::build_body(cfg, req)
}

/// Stream with the OpenAI parser plus OpenRouter headers.
pub async fn stream(
    cfg: &ModelConfig,
    req: LlmRequest,
    tx: Sender<StreamEvent>,
) -> std::result::Result<(), LlmError> {
    openai::stream_with_headers(cfg, req, tx, &extra_headers()).await
}

/// `GET {base_url}/key`: how much the key may still spend.
/// One line, so a failed request is easy to show.
pub async fn budget(cfg: &ModelConfig) -> anyhow::Result<String> {
    if cfg.provider != "openrouter" {
        anyhow::bail!("budget: only available for the openrouter provider");
    }
    let url = format!("{}/key", cfg.base_url.trim_end_matches('/'));
    let settings = RetrySettings::from_env();
    let client = reqwest::Client::builder()
        .connect_timeout(settings.connect())
        .build()
        .map_err(|err| anyhow::anyhow!("budget: cannot build HTTP client: {err}"))?;
    let response = tokio::time::timeout(
        settings.idle,
        client.get(&url).bearer_auth(&cfg.api_key).send(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("budget: request timed out"))?
    .map_err(|err| anyhow::anyhow!("budget: request failed: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("budget: request failed with HTTP {}", status.as_u16());
    }
    let value: Value = tokio::time::timeout(settings.idle, response.json())
        .await
        .map_err(|_| anyhow::anyhow!("budget: reading the response timed out"))?
        .map_err(|err| anyhow::anyhow!("budget: invalid response: {err}"))?;
    Ok(budget_line(&value))
}

/// `limit_remaining=... limit=... usage_daily=...` from the `/key` payload.
/// A null `limit_remaining` means the key has no spending limit.
pub fn budget_line(value: &Value) -> String {
    let data = value.get("data").unwrap_or(value);
    format!(
        "limit_remaining={} limit={} usage_daily={}",
        amount(data.get("limit_remaining"), "no limit"),
        amount(data.get("limit"), "none"),
        amount(data.get("usage_daily"), "$0.0000"),
    )
}

fn amount(value: Option<&Value>, missing: &str) -> String {
    match value {
        Some(Value::Number(number)) => number
            .as_f64()
            .map(|n| format!("${n:.4}"))
            .unwrap_or_else(|| missing.to_string()),
        _ => missing.to_string(),
    }
}

#[cfg(test)]
#[path = "openrouter_tests.rs"]
mod tests;
