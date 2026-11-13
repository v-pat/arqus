//! Pure prompt input: characters, cursor, history navigation and slash hints.
//!
//! The buffer is `Vec<char>` plus a cursor index. Never a byte index: a
//! `String` index is a byte position and breaks on non-ASCII text.

use super::history::History;
pub(super) use super::paste::marker_index;

pub use super::paste::{paste_label, LinePart};

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
    /// Full text of every collapsed paste, in marker order.
    pub(super) pastes: Vec<String>,
    /// Last killed text, for `Ctrl+Y`.
    pub(super) yank: String,
}

impl InputState {
    pub fn new() -> InputState {
        InputState {
            chars: Vec::new(),
            cursor: 0,
            history: History::new(),
            hist_index: None,
            draft: Vec::new(),
            pastes: Vec::new(),
            yank: String::new(),
        }
    }

    pub fn with_history(history: History) -> InputState {
        InputState {
            history,
            ..InputState::new()
        }
    }

    /// The full text, with collapsed pastes expanded again.
    pub fn text(&self) -> String {
        let mut out = String::with_capacity(self.chars.len());
        for &c in &self.chars {
            match marker_index(c) {
                Some(index) => {
                    if let Some(paste) = self.pastes.get(index) {
                        out.push_str(paste);
                    }
                }
                None => out.push(c),
            }
        }
        out
    }

    pub fn set_text(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
        self.pastes.clear();
        self.reset_nav();
    }

    /// Hand the text to the caller and empty the box.
    pub fn take(&mut self) -> String {
        let text = self.text();
        self.chars.clear();
        self.cursor = 0;
        self.pastes.clear();
        self.reset_nav();
        text
    }

    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
        self.pastes.clear();
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
}

impl Default for InputState {
    fn default() -> Self {
        InputState::new()
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
