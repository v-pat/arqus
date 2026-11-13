//! Editing commands that remove text: kill, yank, paste and `@` replacement.

use super::input::InputState;
use super::paste::{marker, marker_index, LARGE_PASTE_CHARS};

impl InputState {
    /// Ctrl+W: delete the word before the cursor, whitespace first.
    pub fn delete_word(&mut self) {
        let mut start = self.cursor;
        while start > 0 && self.chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !self.chars[start - 1].is_whitespace() {
            start -= 1;
        }
        self.cut(start, self.cursor);
    }

    /// Ctrl+U: delete from the line start to the cursor.
    pub fn kill_to_start(&mut self) {
        let start = self.line_start();
        self.cut(start, self.cursor);
    }

    /// Ctrl+K: delete from the cursor to the line end, not the newline.
    pub fn kill_to_end(&mut self) {
        let end = self.line_end();
        self.cut(self.cursor, end);
    }

    /// Ctrl+Y: paste the last killed text at the cursor.
    pub fn yank(&mut self) {
        if self.yank.is_empty() {
            return;
        }
        let text = self.yank.clone();
        self.insert_text(&text);
    }

    /// Bracketed paste. A large paste is kept whole but drawn as one marker.
    pub fn insert_paste(&mut self, text: &str) {
        if text.chars().count() > LARGE_PASTE_CHARS {
            let index = self.pastes.len();
            self.pastes.push(text.to_string());
            self.chars.insert(self.cursor, marker(index));
            self.cursor += 1;
            self.reset_nav();
        } else {
            self.insert_text(text);
        }
    }

    /// Replace a char range, for the `@` completion.
    pub fn replace_range(&mut self, start: usize, end: usize, text: &str) {
        let start = start.min(self.chars.len());
        let end = end.min(self.chars.len()).max(start);
        self.chars.drain(start..end);
        self.cursor = start;
        self.insert_text(text);
    }

    /// Remove a range, remembering it for `Ctrl+Y`.
    fn cut(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        self.yank = self.range_text(start, end);
        self.chars.drain(start..end);
        self.cursor = start;
        self.reset_nav();
    }

    /// Text of a char range, with collapsed pastes expanded.
    fn range_text(&self, start: usize, end: usize) -> String {
        let mut out = String::new();
        let from = start.min(self.chars.len());
        let to = end.min(self.chars.len());
        for &c in &self.chars[from..to] {
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
}
