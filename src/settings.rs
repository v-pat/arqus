//! User settings: `~/.pirs/settings.json` and `<cwd>/.pirs/settings.json`.
//!
//! Precedence, highest first: CLI flag, environment variable, project settings,
//! global settings, built-in default. The API key is never stored here; it comes
//! only from `PIRS_API_KEY`.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ai::{default_base_url, default_context_window, split_model_spec, ModelConfig};
use crate::compaction::CompactionSettings;

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
    /// Extra skill folders. `~` is the home folder; a relative path is
    /// relative to the project.
    pub skill_folders: Vec<String>,
}

/// The compaction keys inside settings.json.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionOptions {
    pub enabled: Option<bool>,
    pub reserve_tokens: Option<u64>,
    pub keep_recent_tokens: Option<u64>,
}

impl Settings {
    /// Settings for `cwd`, using the real home folder.
    pub fn load(cwd: &Path) -> Settings {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        Settings::load_in(&home, cwd)
    }

    /// Same as `load`, but with an explicit home folder. Tests use this.
    /// A missing or broken file is skipped; a broken file prints a warning.
    pub fn load_in(home: &Path, cwd: &Path) -> Settings {
        let mut out = Settings::default();
        let paths = [
            home.join(".pirs").join("settings.json"),
            cwd.join(".pirs").join("settings.json"),
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
        self.skill_folders.extend(other.skill_folders);
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
}

/// Environment values, captured once so model resolution is easy to test.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub context_window: Option<u64>,
    pub api_key: Option<String>,
}

impl EnvOverrides {
    pub fn from_env() -> Self {
        Self {
            provider: env_nonempty("PIRS_PROVIDER"),
            model: env_nonempty("PIRS_MODEL"),
            base_url: env_nonempty("PIRS_BASE_URL"),
            context_window: env_nonempty("PIRS_CONTEXT_WINDOW").and_then(|text| text.parse().ok()),
            api_key: env_nonempty("PIRS_API_KEY"),
        }
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
