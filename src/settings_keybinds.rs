//! One `keybinds` value from `settings.json`: a string or an array.

use serde::{Deserialize, Serialize};

/// A string (`"ctrl+k"` or `"alt+enter,ctrl+j"`) or an array of keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyBindSpec {
    One(String),
    Many(Vec<String>),
}

impl KeyBindSpec {
    /// The key strings, in the order written.
    pub fn keys(&self) -> Vec<String> {
        let parts: Vec<&str> = match self {
            KeyBindSpec::One(text) => text.split(',').collect(),
            KeyBindSpec::Many(keys) => keys.iter().map(String::as_str).collect(),
        };
        parts
            .into_iter()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    }
}
