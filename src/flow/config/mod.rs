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
    let trimmed = api_key_env.trim();
    if trimmed.is_empty() {
        return Ok("local".to_string());
    }
    match std::env::var(trimmed) {
        Ok(key) if !key.is_empty() => {
            crate::debug::register_redacted_key(&key);
            Ok(key)
        }
        _ => Err(FlowError::MissingApiKeyEnv(trimmed.to_string())),
    }
}

pub fn load_flow_config(
    home: &Path,
    cwd: &Path,
    overrides: &FlowCliOverrides,
) -> Result<ResolvedFlowConfig, FlowError> {
    let mut settings = default_flow_settings();
    let paths = [
        home.join("settings.json"),
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

    let orchestrator_key = resolve_api_key(&orchestrator.api_key_env)?;
    let mut worker_keys = BTreeMap::new();
    for worker in &workers {
        let key = resolve_api_key(&worker.api_key_env)?;
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
