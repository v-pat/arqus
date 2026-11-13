//! Model resolution shared by the TUI and print mode.
//!
//! Order, first hit wins: `--model`, `ARQUS_PROVIDER` / `ARQUS_MODEL`,
//! `state.json`, project settings, global settings. The key comes from the
//! provider variable, then `ARQUS_API_KEY`, then `auth.json`.

use std::fmt;

use crate::ai::{
    default_base_url, default_context_window, is_local_base_url, split_model_spec, ModelConfig,
};
use crate::auth::AuthStore;
use crate::catalog::{self, resolve_key};
use crate::settings::{EnvOverrides, Settings};
use crate::state::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    NoProvider,
    NoModel { provider: String },
    NoKey { provider: String },
    BadModel(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NoProvider => f.write_str(
                "No provider connected. Run arqus and use /connect, or set OPENROUTER_API_KEY (or another provider's variable).",
            ),
            ConfigError::NoModel { provider } => write!(
                f,
                "no model configured for {provider}. Run arqus and use /models, or set ARQUS_MODEL."
            ),
            ConfigError::NoKey { provider } => {
                let variable = catalog::find(provider)
                    .and_then(|info| info.env_vars.first().copied())
                    .unwrap_or(catalog::UNIVERSAL_ENV);
                write!(
                    f,
                    "no API key for {provider}. Run arqus and use /connect, or set {variable}."
                )
            }
            ConfigError::BadModel(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Resolve the model from every source. Pure: the caller passes the snapshots.
pub fn resolve_model(
    cli: Option<&str>,
    env: &EnvOverrides,
    state: &State,
    settings: &Settings,
    auth: &AuthStore,
) -> Result<ModelConfig, ConfigError> {
    let cli_pair = cli.map(parse_spec).transpose()?;
    let settings_pair = settings.model.as_deref().map(parse_spec).transpose()?;
    let state_pair = state
        .model
        .as_ref()
        .map(|model| (model.provider.clone(), model.id.clone()));

    let provider = cli_pair
        .as_ref()
        .map(|pair| pair.0.clone())
        .or_else(|| env.provider.clone())
        .or_else(|| state_pair.as_ref().map(|pair| pair.0.clone()))
        .or_else(|| settings_pair.as_ref().map(|pair| pair.0.clone()))
        .ok_or(ConfigError::NoProvider)?;

    let model = cli_pair
        .as_ref()
        .map(|pair| pair.1.clone())
        .or_else(|| env.model.clone())
        .or_else(|| {
            state_pair
                .as_ref()
                .filter(|pair| pair.0 == provider)
                .map(|pair| pair.1.clone())
        })
        .or_else(|| {
            settings_pair
                .as_ref()
                .filter(|pair| pair.0 == provider)
                .map(|pair| pair.1.clone())
        })
        .filter(|model| !model.trim().is_empty())
        .ok_or_else(|| ConfigError::NoModel {
            provider: provider.clone(),
        })?;

    build(provider, model, env, state, settings, auth)
}

/// Build a config for one explicit provider and model, used by the model picker.
pub fn resolve_for(
    provider: &str,
    model: &str,
    env: &EnvOverrides,
    state: &State,
    settings: &Settings,
    auth: &AuthStore,
) -> Result<ModelConfig, ConfigError> {
    if provider.trim().is_empty() {
        return Err(ConfigError::NoProvider);
    }
    if model.trim().is_empty() {
        return Err(ConfigError::NoModel {
            provider: provider.to_string(),
        });
    }
    build(
        provider.to_string(),
        model.to_string(),
        env,
        state,
        settings,
        auth,
    )
}

fn build(
    provider: String,
    model: String,
    env: &EnvOverrides,
    state: &State,
    settings: &Settings,
    auth: &AuthStore,
) -> Result<ModelConfig, ConfigError> {
    let info = catalog::find(&provider);
    let base_url = env
        .base_url
        .clone()
        .or_else(|| state.base_urls.get(&provider).cloned())
        .or_else(|| settings.base_url.clone())
        .or_else(|| {
            info.map(|info| info.base_url.to_string())
                .filter(|url| !url.is_empty())
        })
        .unwrap_or_else(|| default_base_url(&provider).to_string());
    let context_window = env
        .context_window
        .or(settings.context_window)
        .unwrap_or_else(|| default_context_window(&provider));

    let key = match info {
        Some(info) => resolve_key(info, &|name| env.key(name), auth).map(|(key, _)| key),
        None => env
            .key(catalog::UNIVERSAL_ENV)
            .or_else(|| auth.get_key(&provider).map(str::to_string)),
    };
    let needs_key = info.map(|info| info.needs_key).unwrap_or(true);
    if needs_key && key.as_deref().unwrap_or("").trim().is_empty() && !is_local_base_url(&base_url)
    {
        return Err(ConfigError::NoKey { provider });
    }

    Ok(ModelConfig {
        provider,
        model,
        base_url,
        api_key: key.unwrap_or_default(),
        context_window,
    })
}

fn parse_spec(spec: &str) -> Result<(String, String), ConfigError> {
    split_model_spec(spec).map_err(|err| ConfigError::BadModel(format!("{err:#}")))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
