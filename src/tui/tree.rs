//! The `/tree` picker state and the actions it performs.

use crate::tree::{path_text, rows, TreeRow};

use super::app::App;

#[derive(Debug, Clone, Default)]
pub struct TreeState {
    pub rows: Vec<TreeRow>,
    pub selected: usize,
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
        self.tree = Some(TreeState { rows, selected: 0 });
        true
    }

    pub fn move_tree(&mut self, delta: isize) {
        if let Some(state) = &mut self.tree {
            let last = state.rows.len().saturating_sub(1);
            let next = state.selected as isize + delta;
            state.selected = next.clamp(0, last as isize) as usize;
        }
    }

    pub fn cancel_tree(&mut self) {
        self.tree = None;
    }

    /// Move the session leaf to the selected entry. Returns the new path.
    pub fn select_tree(&mut self) -> Option<String> {
        let state = self.tree.take()?;
        let id = state.rows.get(state.selected)?.id.clone();

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
