//! The `/tree` picker state and the actions it performs.

use crate::tree::{path_text, rows, TreeRow};

use super::app::App;
use super::picker::{Item, PKey, Picker};

/// The session tree as a filterable list of entry ids.
#[derive(Debug, Clone)]
pub struct TreeState {
    pub picker: Picker<String>,
}

fn label(row: &TreeRow) -> String {
    format!("{}{}: {}", "  ".repeat(row.depth), row.role, row.preview)
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
            .map(|row| {
                Item::new(label(row), row.id.clone())
                    .detail(row.id.clone())
                    .current(row.on_path)
            })
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

        let Some(session) = self.session.as_mut() else {
            self.info("no session");
            return None;
        };
        match session.set_leaf(&id) {
            Ok(()) => {
                let path = path_text(&session.entries, session.leaf_id.as_deref());
                self.status = "tree".to_string();
                Some(path)
            }
            Err(err) => {
                let message = format!("cannot move to {id}: {err:#}");
                self.info(message);
                None
            }
        }
    }
}
