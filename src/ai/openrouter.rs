//! OpenRouter adapter. Its wire format is OpenAI-compatible, so conversion
//! and SSE parsing stay in `openai.rs`. This module owns the provider name,
//! the default endpoint, and the optional attribution headers.

use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc::Sender;

use super::openai;
use super::{LlmRequest, ModelConfig, StreamEvent};

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
pub async fn stream(cfg: &ModelConfig, req: LlmRequest, tx: Sender<StreamEvent>) -> Result<()> {
    openai::stream_with_headers(cfg, req, tx, &extra_headers()).await
}

#[cfg(test)]
#[path = "openrouter_tests.rs"]
mod tests;
