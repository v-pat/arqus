pub mod anthropic;
pub mod mock;
pub mod openai;
pub mod openrouter;
pub mod sse;

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::Sender;

use crate::types::Message;

mod error;
pub use error::{
    classify_http, friendly_error, is_local_base_url, retry_delay, validate_config, LlmError,
    RetrySettings,
};

/// One tool the model may call. `parameters` is a JSON schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Everything one model call needs. Provider neutral.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub max_tokens: Option<u32>,
}

/// Connection settings for one OpenAI-compatible endpoint.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub context_window: u64,
}

impl ModelConfig {
    /// Read settings from the environment. The key is never printed.
    pub fn from_env() -> Self {
        let provider = env_or("ARQUS_PROVIDER", "openai");
        let context_window = std::env::var("ARQUS_CONTEXT_WINDOW")
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or_else(|| default_context_window(&provider));
        Self {
            model: env_or("ARQUS_MODEL", ""),
            base_url: std::env::var("ARQUS_BASE_URL")
                .unwrap_or_else(|_| default_base_url(&provider).to_string()),
            api_key: env_or("ARQUS_API_KEY", ""),
            context_window,
            provider,
        }
    }

    /// Parse `provider:model`, keeping the other settings from the environment.
    /// An explicit `ARQUS_BASE_URL` or `ARQUS_CONTEXT_WINDOW` still wins.
    pub fn from_model_spec(spec: &str) -> anyhow::Result<Self> {
        let (provider, model) = split_model_spec(spec)?;
        let mut cfg = Self::from_env();
        cfg.provider = provider;
        cfg.model = model;
        if std::env::var("ARQUS_BASE_URL").is_err() {
            cfg.base_url = default_base_url(&cfg.provider).to_string();
        }
        if std::env::var("ARQUS_CONTEXT_WINDOW").is_err() {
            cfg.context_window = default_context_window(&cfg.provider);
        }
        Ok(cfg)
    }
}

/// Split `provider:model` into its two trimmed parts.
pub fn split_model_spec(spec: &str) -> anyhow::Result<(String, String)> {
    use anyhow::Context;
    let (provider, model) = spec
        .split_once(':')
        .with_context(|| format!("model must look like provider:model, got {spec:?}"))?;
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        anyhow::bail!("model must look like provider:model, got {spec:?}");
    }
    Ok((provider.to_string(), model.to_string()))
}

/// Default endpoint for each provider. Any other name is OpenAI-compatible.
pub fn default_base_url(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "https://api.anthropic.com/v1",
        // OpenRouter, OpenAI, and every local OpenAI-compatible server.
        _ => openrouter::DEFAULT_BASE_URL,
    }
}

pub fn default_context_window(provider: &str) -> u64 {
    match provider {
        "anthropic" => 200_000,
        _ => 128_000,
    }
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Debug is written by hand so the API key never reaches a log line.
impl fmt::Debug for ModelConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("context_window", &self.context_window)
            .finish()
    }
}

/// One event from a streaming model call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StreamEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolCall {
        id: String,
        name: String,
        args: Value,
    },
    Done(Message),
    Error(String),
    /// Progress line, for example before a retry. Not part of the answer.
    Notice(String),
}

/// The provider set. Add a variant instead of a trait object.
#[derive(Debug, Clone)]
pub enum Llm {
    OpenAi(ModelConfig),
    OpenRouter(ModelConfig),
    Anthropic(ModelConfig),
    Mock(Arc<Mutex<VecDeque<Vec<StreamEvent>>>>),
    /// Mock that also records every request it receives, for tests.
    MockCapture(
        Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
        Arc<Mutex<Vec<String>>>,
    ),
}

impl Llm {
    /// Pick the adapter from the provider name. Unknown names stay
    /// OpenAI-compatible, which covers most local servers.
    pub fn from_config(cfg: ModelConfig) -> Llm {
        match cfg.provider.as_str() {
            "anthropic" => Llm::Anthropic(cfg),
            "openrouter" => Llm::OpenRouter(cfg),
            _ => Llm::OpenAi(cfg),
        }
    }

    async fn stream_once(
        &self,
        req: LlmRequest,
        tx: Sender<StreamEvent>,
    ) -> std::result::Result<(), LlmError> {
        match self {
            Llm::OpenAi(cfg) => openai::stream(cfg, req, tx).await,
            Llm::OpenRouter(cfg) => openrouter::stream(cfg, req, tx).await,
            Llm::Anthropic(cfg) => anthropic::stream(cfg, req, tx).await,
            Llm::Mock(queue) => mock::stream(queue.clone(), req, tx)
                .await
                .map_err(|err| LlmError::Other(format!("{err:#}"))),
            Llm::MockCapture(queue, seen) => {
                mock::stream_capture(queue.clone(), seen.clone(), req, tx)
                    .await
                    .map_err(|err| LlmError::Other(format!("{err:#}")))
            }
        }
    }

    /// Stream one model call into `tx`, retrying only when the attempt
    /// delivered no content yet. A retry after output could repeat a tool
    /// call, so a failure with partial output ends the request instead.
    ///
    /// Rust note: `Arc<Mutex<T>>` shares one value between many owners.
    /// `Arc` makes the clone cheap and keeps it alive; `Mutex` makes the
    /// mutation safe. The mock queue needs both, because tests reuse it.
    pub async fn stream(&self, req: LlmRequest, tx: Sender<StreamEvent>) -> anyhow::Result<()> {
        let settings = RetrySettings::from_env();
        let mut attempt: u32 = 0;
        loop {
            let (inner_tx, mut inner_rx) = tokio::sync::mpsc::channel(64);
            // The thread that owns the parser runs as its own future, so a
            // half-finished attempt can be dropped cleanly.
            let run = self.stream_once(req.clone(), inner_tx);
            tokio::pin!(run);
            let mut outcome: Option<std::result::Result<(), LlmError>> = None;
            let mut delivered = false;

            loop {
                tokio::select! {
                    result = &mut run, if outcome.is_none() => {
                        outcome = Some(result);
                    }
                    event = inner_rx.recv() => match event {
                        Some(event) => {
                            if is_content(&event) {
                                delivered = true;
                            }
                            if tx.send(event).await.is_err() {
                                return Ok(());
                            }
                        }
                        // The producer future dropped its sender, so it has
                        // returned. Its result is read below, outside the loop.
                        None => break,
                    },
                }
            }
            if outcome.is_none() {
                outcome = Some(run.as_mut().await);
            }

            let error = match outcome.unwrap_or(Ok(())) {
                Ok(()) => return Ok(()),
                Err(error) => error,
            };
            if delivered {
                return Err(anyhow::Error::new(error));
            }
            let delay = if attempt < settings.max_retries {
                retry_delay(&error, attempt, settings.base_ms)
            } else {
                None
            };
            let Some(delay) = delay else {
                return Err(anyhow::Error::new(error));
            };
            attempt += 1;
            let notice = format!(
                "{}, retrying in {} (attempt {}/{})",
                error.label(),
                wait_text(delay),
                attempt,
                settings.max_retries
            );
            crate::debug::log("retry", serde_json::json!({"notice": notice}));
            if tx.send(StreamEvent::Notice(notice)).await.is_err() {
                return Ok(());
            }
            tokio::time::sleep(delay).await;
        }
    }
}

/// Text or a tool call means the model already produced something.
fn is_content(event: &StreamEvent) -> bool {
    match event {
        StreamEvent::TextDelta(text) | StreamEvent::ThinkingDelta(text) => !text.is_empty(),
        StreamEvent::ToolCall { .. } => true,
        _ => false,
    }
}

fn wait_text(delay: Duration) -> String {
    if delay.as_secs() >= 1 {
        format!("{}s", delay.as_secs())
    } else {
        format!("{}ms", delay.as_millis())
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
