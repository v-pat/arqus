//! The `/` menu: matching, selection and the Esc dismissal.
//!
//! Pure state. `update` calls `sync` after every key, so the popup always
//! reflects the input box.

use super::commands::Command;
use super::fuzzy::fuzzy_match;

pub const MAX_MATCHES: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct SlashState {
    pub open: bool,
    /// Indexes into `App::commands`, best match first.
    pub matched: Vec<usize>,
    pub selected: usize,
    /// Text that Esc closed the popup for; typing anything else reopens it.
    dismissed: Option<String>,
}

impl SlashState {
    pub fn selected_command<'a>(&self, commands: &'a [Command]) -> Option<&'a Command> {
        self.matched
            .get(self.selected)
            .and_then(|index| commands.get(*index))
    }

    pub fn select_next(&mut self) {
        if !self.matched.is_empty() {
            self.selected = (self.selected + 1) % self.matched.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.matched.is_empty() {
            self.selected = if self.selected == 0 {
                self.matched.len() - 1
            } else {
                self.selected - 1
            };
        }
    }
}

/// The query after `/`, when the input is a single slash word.
pub fn query_of(text: &str) -> Option<String> {
    if !text.starts_with('/') || text.contains(['\n', ' ', '\t']) {
        return None;
    }
    Some(text[1..].to_string())
}

/// Fuzzy match on names and aliases, best first, capped at `MAX_MATCHES`.
pub fn matches(query: &str, commands: &[Command]) -> Vec<usize> {
    let mut ranked: Vec<(usize, i32)> = commands
        .iter()
        .enumerate()
        .filter(|(_, command)| command.slash)
        .filter_map(|(index, command)| {
            let mut best: Option<i32> = None;
            for name in std::iter::once(&command.name).chain(command.aliases.iter()) {
                if let Some(found) = fuzzy_match(query, name) {
                    best = Some(best.map_or(found.score, |score| score.max(found.score)));
                }
            }
            best.map(|score| (index, score))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(MAX_MATCHES)
        .map(|(index, _)| index)
        .collect()
}

/// Recompute the popup from the input text.
pub fn sync(state: &mut SlashState, text: &str, commands: &[Command]) {
    let Some(query) = query_of(text) else {
        close(state);
        return;
    };
    if state.dismissed.as_deref() == Some(text) {
        close(state);
        return;
    }
    state.matched = matches(&query, commands);
    state.open = true;
    if state.selected >= state.matched.len() {
        state.selected = state.matched.len().saturating_sub(1);
    }
}

/// Esc: hide the popup but keep the text. It reopens when the text changes.
pub fn dismiss(state: &mut SlashState, text: &str) {
    state.dismissed = Some(text.to_string());
    close(state);
}

fn close(state: &mut SlashState) {
    state.open = false;
    state.matched.clear();
    state.selected = 0;
}

#[cfg(test)]
#[path = "slash_tests.rs"]
mod tests;
