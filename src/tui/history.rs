//! Prompt history: `<ARQUS_HOME>/history.jsonl`, one JSON string per line.

use std::path::PathBuf;

/// At most this many prompts are kept; the oldest go first.
pub const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Default)]
pub struct History {
    entries: Vec<String>,
    path: Option<PathBuf>,
}

impl History {
    /// An in-memory history with no file attached. Tests use this.
    pub fn new() -> History {
        History::default()
    }

    /// A history that saves to `path`.
    pub fn with_path(path: PathBuf) -> History {
        History {
            entries: Vec::new(),
            path: Some(path),
        }
    }

    /// Read every valid JSON string line. Missing or bad files start empty.
    pub fn load(path: PathBuf) -> History {
        let mut history = History::with_path(path.clone());
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(item) = serde_json::from_str::<String>(line) {
                if !item.is_empty() {
                    history.entries.push(item);
                }
            }
        }
        trim(&mut history.entries);
        history
    }

    /// Add one prompt. Empty text and consecutive duplicates are skipped.
    /// Returns true when the list changed.
    pub fn remember(&mut self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return false;
        }
        if self.entries.last().map(String::as_str) == Some(text) {
            return false;
        }
        self.entries.push(text.to_string());
        trim(&mut self.entries);
        self.save();
        true
    }

    pub fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut text = String::new();
        for entry in &self.entries {
            if let Ok(line) = serde_json::to_string(entry) {
                text.push_str(&line);
                text.push('\n');
            }
        }
        let _ = std::fs::write(path, text);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(String::as_str)
    }

    /// Oldest first, as stored.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

/// Keep only the newest `MAX_ENTRIES` prompts.
fn trim(entries: &mut Vec<String>) {
    if entries.len() > MAX_ENTRIES {
        let drop = entries.len() - MAX_ENTRIES;
        entries.drain(0..drop);
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
