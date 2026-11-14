//! User settings: `<ARQUS_HOME>/settings.json` and `<cwd>/.arqus/settings.json`.
//!
//! Precedence, highest first: CLI flag, environment variable, project settings,
//! global settings, built-in default. The API key is never stored here; it comes
//! only from `ARQUS_API_KEY`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ai::{default_base_url, default_context_window, split_model_spec, ModelConfig};
use crate::compaction::CompactionSettings;
use crate::cost::Limits;

/// `approval` in settings.json: ask (default), auto or plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalMode {
    #[default]
    Ask,
    Auto,
    Plan,
}

/// One settings.json file. Every field is optional, so a project file can
/// override a single key and inherit the rest from the global file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `provider:model`, the same shape as the `--model` flag.
    pub model: Option<String>,
    /// Endpoint override, for example a local server.
    pub base_url: Option<String>,
    /// Context window in tokens.
    pub context_window: Option<u64>,
    /// Compaction numbers.
    pub compaction: CompactionOptions,
    /// Cost and token limits.
    pub limits: LimitOptions,
    /// Extra skill folders. `~` is the home folder; a relative path is
    /// relative to the project.
    pub skill_folders: Vec<String>,
    /// `ask` (default) or `auto`. `--yolo` also means auto.
    pub approval: Option<ApprovalMode>,
    /// Folders outside the project where write and edit are allowed.
    /// `~` is the home folder; a relative path is relative to the project.
    pub allow_paths: Vec<String>,
    /// Key overrides: action name to one key or a comma-separated list.
    pub keybinds: BTreeMap<String, KeyBindSpec>,
}

pub use crate::settings_keybinds::KeyBindSpec;

/// The compaction keys inside settings.json.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionOptions {
    pub enabled: Option<bool>,
    pub reserve_tokens: Option<u64>,
    pub keep_recent_tokens: Option<u64>,
}

/// The `limits` object inside settings.json.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitOptions {
    pub max_cost_per_run: Option<f64>,
    pub max_cost_per_session: Option<f64>,
    pub max_rounds: Option<usize>,
    pub max_output_tokens: Option<u32>,
    pub max_tokens_per_run: Option<u64>,
}

impl Settings {
    /// Settings for `cwd`, using the real arqus home folder.
    pub fn load(cwd: &Path) -> Settings {
        Settings::load_in(&crate::home::arqus_home(), cwd)
    }

    /// Same as `load`, but with an explicit arqus home folder. Tests use this.
    /// A missing or broken file is skipped; a broken file prints a warning.
    pub fn load_in(home: &Path, cwd: &Path) -> Settings {
        let mut out = Settings::default();
        let paths = [
            home.join("settings.json"),
            cwd.join(".arqus").join("settings.json"),
        ];
        for path in paths {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            match serde_json::from_str::<Settings>(&text) {
                Ok(found) => out.overlay(found),
                Err(err) => eprintln!("warning: ignoring {}: {err}", path.display()),
            }
        }
        out
    }

    /// Apply another settings file on top of this one. Project wins on
    /// scalars; skill folders accumulate.
    fn overlay(&mut self, other: Settings) {
        if other.model.is_some() {
            self.model = other.model;
        }
        if other.base_url.is_some() {
            self.base_url = other.base_url;
        }
        if other.context_window.is_some() {
            self.context_window = other.context_window;
        }
        if other.compaction.enabled.is_some() {
            self.compaction.enabled = other.compaction.enabled;
        }
        if other.compaction.reserve_tokens.is_some() {
            self.compaction.reserve_tokens = other.compaction.reserve_tokens;
        }
        if other.compaction.keep_recent_tokens.is_some() {
            self.compaction.keep_recent_tokens = other.compaction.keep_recent_tokens;
        }
        if other.limits.max_cost_per_run.is_some() {
            self.limits.max_cost_per_run = other.limits.max_cost_per_run;
        }
        if other.limits.max_cost_per_session.is_some() {
            self.limits.max_cost_per_session = other.limits.max_cost_per_session;
        }
        if other.limits.max_rounds.is_some() {
            self.limits.max_rounds = other.limits.max_rounds;
        }
        if other.limits.max_output_tokens.is_some() {
            self.limits.max_output_tokens = other.limits.max_output_tokens;
        }
        if other.limits.max_tokens_per_run.is_some() {
            self.limits.max_tokens_per_run = other.limits.max_tokens_per_run;
        }
        if other.approval.is_some() {
            self.approval = other.approval;
        }
        self.skill_folders.extend(other.skill_folders);
        self.allow_paths.extend(other.allow_paths);
        self.keybinds.extend(other.keybinds);
    }

    /// Built-in compaction defaults with the settings keys applied.
    pub fn compaction_settings(&self, base: CompactionSettings) -> CompactionSettings {
        let mut out = base;
        if let Some(value) = self.compaction.enabled {
            out.enabled = value;
        }
        if let Some(value) = self.compaction.reserve_tokens {
            out.reserve_tokens = value;
        }
        if let Some(value) = self.compaction.keep_recent_tokens {
            out.keep_recent_tokens = value;
        }
        out
    }

    /// Built-in limits with the settings keys applied.
    pub fn limits(&self, base: Limits) -> Limits {
        let mut out = base;
        if let Some(value) = self.limits.max_cost_per_run {
            out.max_cost_per_run = value;
        }
        if let Some(value) = self.limits.max_cost_per_session {
            out.max_cost_per_session = value;
        }
        if let Some(value) = self.limits.max_rounds {
            out.max_rounds = value;
        }
        if let Some(value) = self.limits.max_output_tokens {
            out.max_output_tokens = value;
        }
        if let Some(value) = self.limits.max_tokens_per_run {
            out.max_tokens_per_run = value;
        }
        out
    }
}

/// Environment values, captured once so model resolution is easy to test.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub context_window: Option<u64>,
    pub api_key: Option<String>,
    /// Provider key variables, captured once so key resolution is testable.
    pub keys: BTreeMap<String, String>,
}

impl EnvOverrides {
    pub fn from_env() -> Self {
        let mut keys = BTreeMap::new();
        for info in crate::catalog::PROVIDERS {
            for name in info.env_vars {
                if let Some(value) = env_nonempty(name) {
                    keys.insert((*name).to_string(), value);
                }
            }
        }
        if let Some(value) = env_nonempty(crate::catalog::UNIVERSAL_ENV) {
            keys.insert(crate::catalog::UNIVERSAL_ENV.to_string(), value);
        }
        Self {
            provider: env_nonempty("ARQUS_PROVIDER"),
            model: env_nonempty("ARQUS_MODEL"),
            base_url: env_nonempty("ARQUS_BASE_URL"),
            context_window: env_nonempty("ARQUS_CONTEXT_WINDOW").and_then(|text| text.parse().ok()),
            api_key: env_nonempty("ARQUS_API_KEY"),
            keys,
        }
    }

    /// Value of one key variable, if it was set when the snapshot was taken.
    pub fn key(&self, name: &str) -> Option<String> {
        self.keys.get(name).cloned().or_else(|| {
            (name == crate::catalog::UNIVERSAL_ENV)
                .then(|| self.api_key.clone())
                .flatten()
        })
    }

    /// The CLI flag beats the environment; the environment beats settings;
    /// settings beat built-in defaults.
    pub fn resolve(
        &self,
        settings: &Settings,
        cli_spec: Option<&str>,
    ) -> anyhow::Result<ModelConfig> {
        let (cli_provider, cli_model) = split_optional(cli_spec)?;
        let (set_provider, set_model) = split_optional(settings.model.as_deref())?;

        let provider = cli_provider
            .or_else(|| self.provider.clone())
            .or(set_provider)
            .unwrap_or_else(|| "openai".to_string());
        let model = cli_model
            .or_else(|| self.model.clone())
            .or(set_model)
            .unwrap_or_default();
        let base_url = self
            .base_url
            .clone()
            .or_else(|| settings.base_url.clone())
            .unwrap_or_else(|| default_base_url(&provider).to_string());
        let context_window = self
            .context_window
            .or(settings.context_window)
            .unwrap_or_else(|| default_context_window(&provider));

        Ok(ModelConfig {
            provider,
            model,
            base_url,
            api_key: self.api_key.clone().unwrap_or_default(),
            context_window,
        })
    }
}

fn split_optional(spec: Option<&str>) -> anyhow::Result<(Option<String>, Option<String>)> {
    match spec {
        Some(spec) => {
            let (provider, model) = split_model_spec(spec)?;
            Ok((Some(provider), Some(model)))
        }
        None => Ok((None, None)),
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|text| !text.is_empty())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
