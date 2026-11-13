//! What the TUI remembers between runs: `<ARQUS_HOME>/state.json`.
//!
//! The file is small and never critical. A corrupt file is moved aside and an
//! empty state is used, so a bad edit can never stop the app from starting.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub model: Option<ModelRef>,
    /// Newest first, at most eight entries.
    pub recent: Vec<ModelRef>,
    pub theme: Option<String>,
    pub base_urls: BTreeMap<String, String>,
    /// `Ctrl+T` state: show the thinking text instead of one dim line.
    pub show_thinking: bool,
    /// `Ctrl+O` state: show every tool output expanded.
    pub expand_tool_output: bool,
    /// Mouse capture. Off by default so text selection keeps working.
    pub mouse: bool,
    /// Ring the terminal bell on an approval request and a finished run.
    pub bell: bool,
    /// Mode for a new session: `ask` or `auto`.
    pub default_mode: Option<String>,
    /// Automatic compaction on or off.
    pub compaction: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            model: None,
            recent: Vec::new(),
            theme: None,
            base_urls: BTreeMap::new(),
            show_thinking: false,
            expand_tool_output: false,
            mouse: false,
            bell: true,
            default_mode: None,
            compaction: true,
        }
    }
}

impl State {
    /// Missing file = empty state. A file that cannot be parsed is moved to
    /// `state.json.bad-<ts>` and an empty state is returned.
    pub fn load(path: &Path) -> State {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => return State::default(),
        };
        if text.trim().is_empty() {
            return State::default();
        }
        match serde_json::from_str::<State>(&text) {
            Ok(state) => state,
            Err(_) => {
                let _ = back_up_corrupt(path);
                State::default()
            }
        }
    }

    /// Atomic save: write a temp file, then rename over the target.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, format!("{data}\n"))
            .with_context(|| format!("cannot write {}", tmp.display()))?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Remember a model choice: set it as current and move it to the front of
    /// the recent list, keeping at most eight entries.
    pub fn set_model(&mut self, provider: &str, id: &str) {
        let entry = ModelRef {
            provider: provider.to_string(),
            id: id.to_string(),
        };
        self.model = Some(entry.clone());
        self.recent.retain(|item| item != &entry);
        self.recent.insert(0, entry);
        self.recent.truncate(8);
    }

    /// Recent models for one provider, newest first.
    pub fn recent_for(&self, provider: &str) -> Vec<ModelRef> {
        self.recent
            .iter()
            .filter(|item| item.provider == provider)
            .cloned()
            .collect()
    }
}

fn back_up_corrupt(path: &Path) -> Option<PathBuf> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let bad = path.with_extension(format!("json.bad-{ts}"));
    fs::rename(path, &bad).ok().map(|_| bad)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
