use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::flow::errors::FlowError;

pub mod defaults;
pub mod types;

pub use defaults::*;
pub use types::*;

pub fn resolve_api_key(api_key_env: &str) -> Result<String, FlowError> {
    resolve_api_key_with_context(api_key_env, "", None)
}

pub fn resolve_api_key_with_context(
    api_key_env: &str,
    provider: &str,
    home: Option<&Path>,
) -> Result<String, FlowError> {
    let trimmed = api_key_env.trim();
    if trimmed.is_empty() {
        return Ok("local".to_string());
    }
    // 1. Explicit env var
    if let Ok(key) = std::env::var(trimmed) {
        if !key.trim().is_empty() {
            crate::debug::register_redacted_key(&key);
            return Ok(key);
        }
    }
    // 2. Common fallback env vars
    let provider_env = match provider.to_ascii_lowercase().as_str() {
        "openrouter" => Some("OPENROUTER_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "openai" => Some("OPENAI_API_KEY"),
        _ => None,
    };
    if let Some(penv) = provider_env {
        if let Ok(key) = std::env::var(penv) {
            if !key.trim().is_empty() {
                crate::debug::register_redacted_key(&key);
                return Ok(key);
            }
        }
    }
    if trimmed == "ARQUS_API_KEY" {
        for fallback in ["OPENROUTER_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY"] {
            if let Ok(key) = std::env::var(fallback) {
                if !key.trim().is_empty() {
                    crate::debug::register_redacted_key(&key);
                    return Ok(key);
                }
            }
        }
    }
    // 3. Saved key in auth.json (<home>/auth.json, <home>/.arqus/auth.json, ~/.arqus/auth.json)
    let mut auth_paths = Vec::new();
    if let Some(h) = home {
        auth_paths.push(h.join("auth.json"));
        auth_paths.push(h.join(".arqus").join("auth.json"));
    }
    auth_paths.push(crate::home::arqus_home().join("auth.json"));
    if let Some(user_home) = dirs::home_dir() {
        auth_paths.push(user_home.join(".arqus").join("auth.json"));
    }

    for auth_path in auth_paths {
        if let Ok(store) = crate::auth::AuthStore::load(&auth_path) {
            if !provider.is_empty() {
                if let Some(key) = store.get_key(provider) {
                    if !key.trim().is_empty() {
                        crate::debug::register_redacted_key(key);
                        return Ok(key.to_string());
                    }
                }
            }
            if trimmed == "ARQUS_API_KEY" {
                for prov in ["openrouter", "anthropic", "openai"] {
                    if let Some(key) = store.get_key(prov) {
                        if !key.trim().is_empty() {
                            crate::debug::register_redacted_key(key);
                            return Ok(key.to_string());
                        }
                    }
                }
                if let Some((_, cred)) = store.providers.iter().next() {
                    let crate::auth::Credential::Api { key } = cred;
                    if !key.trim().is_empty() {
                        crate::debug::register_redacted_key(key);
                        return Ok(key.clone());
                    }
                }
            }
        }
    }

    Err(FlowError::MissingApiKeyEnv(trimmed.to_string()))
}

pub fn load_flow_config(
    home: &Path,
    cwd: &Path,
    overrides: &FlowCliOverrides,
) -> Result<ResolvedFlowConfig, FlowError> {
    let mut settings = default_flow_settings();
    let paths = [
        home.join("settings.json"),
        home.join(".arqus").join("settings.json"),
        cwd.join(".arqus").join("settings.json"),
    ];
    for path in paths {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let val: Value = serde_json::from_str(&text)
            .map_err(|e| FlowError::Malformed(format!("{}: {e}", path.display())))?;
        if let Some(flow_val) = val.get("flow") {
            let parsed: FlowSettings = serde_json::from_value(flow_val.clone())
                .map_err(|e| FlowError::Malformed(format!("{}: {e}", path.display())))?;
            overlay_settings(&mut settings, parsed);
        }
    }

    let mut orchestrator = settings
        .orchestrator
        .ok_or_else(|| FlowError::Config("missing orchestrator config".to_string()))?;
    let workers = settings
        .workers
        .ok_or_else(|| FlowError::Config("missing workers config".to_string()))?;
    let mut routing = settings
        .routing
        .ok_or_else(|| FlowError::Config("missing routing config".to_string()))?;
    let mut limits = settings
        .limits
        .ok_or_else(|| FlowError::Config("missing limits config".to_string()))?;
    let sandbox = settings
        .sandbox
        .ok_or_else(|| FlowError::Config("missing sandbox config".to_string()))?;

    if let Some(ref model) = overrides.orchestrator_model {
        orchestrator.model = model.clone();
    }
    if let Some(ref worker) = overrides.worker {
        routing.default_worker = worker.clone();
    }
    if let Some(tokens) = overrides.max_cloud_tokens {
        limits.max_cloud_tokens_per_run = tokens;
    }

    if !workers.iter().any(|w| w.name == routing.default_worker) {
        return Err(FlowError::UnknownWorker(routing.default_worker));
    }

    if !routing.by_category.is_empty() {
        eprintln!("warning: routing.by_category is ignored in v1");
    }

    let orchestrator_key = resolve_api_key_with_context(
        &orchestrator.api_key_env,
        &orchestrator.provider,
        Some(home),
    )?;
    let mut worker_keys = BTreeMap::new();
    for worker in &workers {
        let key = resolve_api_key_with_context(&worker.api_key_env, &worker.provider, Some(home))?;
        worker_keys.insert(worker.name.clone(), key);
    }

    Ok(ResolvedFlowConfig {
        orchestrator,
        workers,
        routing,
        limits,
        sandbox,
        orchestrator_key,
        worker_keys,
    })
}

#[cfg(test)]
mod tests;
