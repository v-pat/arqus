//! Merge `settings.json` key overrides into the default key map.

use std::collections::BTreeMap;

use crate::settings::KeyBindSpec;

use super::actions::Action;
use super::keys::{parse_key, KeyMap};

/// The effective key map plus one notice per rejected or conflicting entry.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    pub keymap: KeyMap,
    pub notices: Vec<String>,
}

/// Apply overrides on top of the defaults. Later entries win a shared key.
pub fn merge(spec: &BTreeMap<String, KeyBindSpec>) -> KeyBindings {
    let mut keymap = KeyMap::defaults();
    let mut notices = Vec::new();
    for (name, definition) in spec {
        let Some(action) = Action::from_name(name) else {
            notices.push(format!("keybinds: unknown action \"{name}\""));
            continue;
        };
        let mut parsed = Vec::new();
        for text in definition.keys() {
            match parse_key(&text) {
                Some(key) => parsed.push(key),
                None => notices.push(format!("keybinds: unparsable key \"{text}\" for {name}")),
            }
        }
        if parsed.is_empty() {
            continue;
        }
        keymap.remove_action(action);
        for key in parsed {
            if let Some(previous) = keymap.replace_key(key, action) {
                if previous != action {
                    notices.push(format!(
                        "keybinds: {} now runs {} (was {})",
                        key.label(),
                        name,
                        previous.name()
                    ));
                }
            }
        }
    }
    KeyBindings { keymap, notices }
}

#[cfg(test)]
#[path = "keybinds_tests.rs"]
mod tests;
