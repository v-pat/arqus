//! The `/tree` picker state and the actions it performs.

use crate::tree::{path_text, rows, TreeRow};

use super::app::App;
use super::picker::{Item, PKey, Picker};

/// The session tree as a filterable list of entry ids.
#[derive(Debug, Clone)]
pub struct TreeState {
    pub picker: Picker<String>,
}

/// Two spaces per level, at most eight levels, then one ellipsis.
pub fn indent(depth: usize) -> String {
    let mut out = "  ".repeat(depth.min(8));
    if depth > 8 {
        out.push_str("… ");
    }
    out
}

fn label(row: &TreeRow) -> String {
    format!("{}{}: {}", indent(row.depth), row.role, row.preview)
}

impl App {
    /// Open the tree view for the current session.
    pub fn start_tree(&mut self) -> bool {
        let Some(session) = self.session.as_ref() else {
            self.info("no session");
            return false;
        };
        let rows = rows(&session.entries, session.leaf_id.as_deref());
        if rows.is_empty() {
            self.info("session is empty");
            return false;
        }
        let items = rows
            .iter()
            .map(|row| Item::new(label(row), row.id.clone()).current(row.on_path))
            .collect();
        self.tree = Some(TreeState {
            picker: Picker::new("tree", items),
        });
        true
    }

    pub fn move_tree(&mut self, delta: isize) {
        if let Some(state) = &mut self.tree {
            let key = if delta < 0 { PKey::Up } else { PKey::Down };
            state.picker.handle_key(key);
        }
    }

    pub fn cancel_tree(&mut self) {
        self.tree = None;
    }
    /// Move the session leaf to the selected entry. Returns the new path.
    pub fn select_tree(&mut self) -> Option<String> {
        let state = self.tree.take()?;
        let id = state.picker.selected_item()?.value.clone();

        let moved = match self.session.as_mut() {
            Some(session) => match session.set_leaf(&id) {
                Ok(()) => {
                    let path = path_text(&session.entries, session.leaf_id.as_deref());
                    Ok(path)
                }
                Err(err) => Err(format!("cannot move to {id}: {err:#}")),
            },
            None => Err("no session".to_string()),
        };
        match moved {
            Ok(path) => {
                self.replay_session();
                self.status = "tree".to_string();
                Some(path)
            }
            Err(message) => {
                self.info(message);
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
