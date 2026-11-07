//! Pure prompt input: characters, cursor, history navigation and slash hints.
//!
//! The buffer is `Vec<char>` plus a cursor index. Never a byte index: a
//! `String` index is a byte position and breaks on non-ASCII text.

use super::history::History;

/// Built-in commands offered by the hint line and Tab completion.
pub const COMMANDS: &[&str] = &[
    "/compact", "/tree", "/fork", "/model", "/cost", "/budget", "/new", "/resume", "/help",
    "/quit", "/exit",
];

/// Prompt box contents plus the editing cursor.
#[derive(Debug, Clone)]
pub struct InputState {
    pub(super) chars: Vec<char>,
    /// Char index, 0 to `chars.len()`.
    pub(super) cursor: usize,
    pub history: History,
    /// `Some(i)` while walking history; `None` while editing the draft.
    pub(super) hist_index: Option<usize>,
    pub(super) draft: Vec<char>,
}

impl InputState {
    pub fn new() -> InputState {
        InputState {
            chars: Vec::new(),
            cursor: 0,
            history: History::new(),
            hist_index: None,
            draft: Vec::new(),
        }
    }

    pub fn with_history(history: History) -> InputState {
        InputState {
            history,
            ..InputState::new()
        }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn set_text(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
        self.reset_nav();
    }

    /// Hand the text to the caller and empty the box.
    pub fn take(&mut self) -> String {
        let text = self.text();
        self.chars.clear();
        self.cursor = 0;
        self.reset_nav();
        text
    }

    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
        self.reset_nav();
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn insert_char(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
        self.reset_nav();
    }

    /// Paste: insert the text exactly, newlines included.
    pub fn insert_text(&mut self, text: &str) {
        for c in text.chars() {
            self.chars.insert(self.cursor, c);
            self.cursor += 1;
        }
        self.reset_nav();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
            self.reset_nav();
        }
    }

    /// Delete the character under the cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
            self.reset_nav();
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    /// Move to the start of the previous word (Alt+Left).
    pub fn word_left(&mut self) {
        let mut i = self.cursor;
        while i > 0 && self.chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !self.chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.cursor = i;
    }

    /// Move to the end of the next word (Alt+Right).
    pub fn word_right(&mut self) {
        let mut i = self.cursor;
        while i < self.chars.len() && !self.chars[i].is_whitespace() {
            i += 1;
        }
        while i < self.chars.len() && self.chars[i].is_whitespace() {
            i += 1;
        }
        self.cursor = i;
    }

    /// Home / Ctrl+A: start of the current line.
    pub fn home(&mut self) {
        self.cursor = self.line_start();
    }

    /// End / Ctrl+E: end of the current line.
    pub fn end(&mut self) {
        self.cursor = self.line_end();
    }

    /// Ctrl+W: delete the word before the cursor, whitespace first.
    pub fn delete_word(&mut self) {
        let mut start = self.cursor;
        while start > 0 && self.chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !self.chars[start - 1].is_whitespace() {
            start -= 1;
        }
        self.chars.drain(start..self.cursor);
        self.cursor = start;
        self.reset_nav();
    }

    /// Ctrl+U: delete from the line start to the cursor.
    pub fn kill_to_start(&mut self) {
        let start = self.line_start();
        self.chars.drain(start..self.cursor);
        self.cursor = start;
        self.reset_nav();
    }

    /// Ctrl+K: delete from the cursor to the line end, not the newline.
    pub fn kill_to_end(&mut self) {
        let end = self.line_end();
        self.chars.drain(self.cursor..end);
        self.reset_nav();
    }

    /// Ctrl+J / Alt+Enter: insert a newline at the cursor.
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Up: previous prompt, only from the first line.
    pub fn history_up(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        let next = match self.hist_index {
            Some(0) => return false,
            Some(i) => i - 1,
            None => {
                self.draft = self.chars.clone();
                self.history.len() - 1
            }
        };
        self.hist_index = Some(next);
        self.load_history(next);
        true
    }

    /// Down: next prompt, then back to the draft that was being edited.
    pub fn history_down(&mut self) -> bool {
        let Some(i) = self.hist_index else {
            return false;
        };
        if i + 1 < self.history.len() {
            self.hist_index = Some(i + 1);
            self.load_history(i + 1);
            return true;
        }
        self.hist_index = None;
        self.chars = std::mem::take(&mut self.draft);
        self.cursor = self.chars.len();
        true
    }

    fn load_history(&mut self, index: usize) {
        self.chars = self
            .history
            .get(index)
            .map(|text| text.chars().collect())
            .unwrap_or_default();
        self.cursor = self.chars.len();
    }

    /// Editing after a recall leaves history mode; the next Up saves a draft.
    pub(super) fn reset_nav(&mut self) {
        self.hist_index = None;
        self.draft.clear();
    }

    pub fn remember(&mut self, text: &str) {
        self.history.remember(text);
    }

    fn slash_query(&self) -> Option<String> {
        let text = self.text();
        if !text.starts_with('/') || text.contains(['\n', ' ', '\t']) {
            return None;
        }
        Some(text)
    }

    /// Commands that match what was typed after `/`.
    pub fn slash_matches(&self) -> Vec<&'static str> {
        let Some(query) = self.slash_query() else {
            return Vec::new();
        };
        COMMANDS
            .iter()
            .copied()
            .filter(|command| command.starts_with(&query))
            .collect()
    }

    /// One-line hint under the box, or `None`.
    pub fn hint(&self) -> Option<String> {
        let matches = self.slash_matches();
        if matches.is_empty() {
            None
        } else {
            Some(matches.join("  "))
        }
    }

    /// Tab: complete when exactly one command matches. Returns true if it did.
    pub fn tab_complete(&mut self) -> bool {
        let matches = self.slash_matches();
        if matches.len() != 1 {
            return false;
        }
        self.set_text(matches[0]);
        true
    }
}

impl Default for InputState {
    fn default() -> Self {
        InputState::new()
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
