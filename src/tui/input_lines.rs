//! Line geometry and the visible window of the prompt box.

use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use super::input::{marker_index, paste_label, InputState, LinePart};

impl InputState {
    pub(super) fn line_start(&self) -> usize {
        self.chars[..self.cursor]
            .iter()
            .rposition(|&c| c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    pub(super) fn line_end(&self) -> usize {
        self.chars[self.cursor..]
            .iter()
            .position(|&c| c == '\n')
            .map(|i| self.cursor + i)
            .unwrap_or(self.chars.len())
    }

    /// Zero-based line holding the cursor.
    pub fn cursor_line(&self) -> usize {
        self.chars[..self.cursor]
            .iter()
            .filter(|&&c| c == '\n')
            .count()
    }

    pub fn line_count(&self) -> usize {
        1 + self.chars.iter().filter(|&&c| c == '\n').count()
    }

    /// Text of one line, without its newline.
    pub fn line_text(&self, line: usize) -> String {
        self.chars
            .split(|&c| c == '\n')
            .nth(line)
            .map(|chars| chars.iter().collect())
            .unwrap_or_default()
    }

    /// Display column of the cursor inside its line (CJK counts 2, tab 4).
    pub fn cursor_column(&self) -> usize {
        self.chars[self.line_start()..self.cursor]
            .iter()
            .map(|&c| self.char_width(c))
            .sum()
    }

    /// One char's display width; a paste marker counts as its label.
    fn char_width(&self, c: char) -> usize {
        match marker_index(c) {
            Some(index) => self
                .pastes
                .get(index)
                .map(|paste| paste_label(paste.chars().count()).width())
                .unwrap_or(1),
            None => match c {
                '\t' => 4,
                c => c.width().unwrap_or(0),
            },
        }
    }

    /// A line split into plain text and collapsed-paste markers.
    pub fn line_parts(&self, line: usize) -> Vec<LinePart> {
        let Some(chars) = self.chars.split(|&c| c == '\n').nth(line) else {
            return Vec::new();
        };
        let mut parts: Vec<LinePart> = Vec::new();
        let mut buf = String::new();
        for &c in chars {
            match marker_index(c) {
                Some(index) => {
                    if !buf.is_empty() {
                        parts.push(LinePart::Text(std::mem::take(&mut buf)));
                    }
                    let chars = self
                        .pastes
                        .get(index)
                        .map(|paste| paste.chars().count())
                        .unwrap_or(0);
                    parts.push(LinePart::Paste(chars));
                }
                None => buf.push(c),
            }
        }
        if !buf.is_empty() {
            parts.push(LinePart::Text(buf));
        }
        parts
    }

    /// Visible line range: at most `max` rows, scrolled to keep the cursor.
    pub fn view(&self, max: usize) -> (usize, usize) {
        let max = max.max(1);
        let total = self.line_count();
        if total <= max {
            return (0, total);
        }
        let start = self.cursor_line().saturating_sub(max - 1).min(total - max);
        (start, start + max)
    }
}
