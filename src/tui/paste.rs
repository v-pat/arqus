//! Collapsed paste markers and the label drawn in the editor.
//!
//! A large paste is stored in `InputState::pastes`; the buffer holds one
//! private-use char per paste, so editing stays cheap.

/// A paste longer than this many characters is stored whole but drawn as one
/// dim marker line, so the box stays readable.
pub const LARGE_PASTE_CHARS: usize = 2000;

/// Private-use characters mark a collapsed paste inside `chars`.
const PASTE_MARK_BASE: u32 = 0xE000;
const PASTE_MARK_COUNT: u32 = 6400;

/// One piece of a rendered input line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinePart {
    Text(String),
    /// Character count of the collapsed paste behind the marker.
    Paste(usize),
}

pub(super) fn marker(index: usize) -> char {
    char::from_u32(PASTE_MARK_BASE + index as u32).unwrap_or('\u{E000}')
}

pub(super) fn marker_index(c: char) -> Option<usize> {
    let code = c as u32;
    if (PASTE_MARK_BASE..PASTE_MARK_BASE + PASTE_MARK_COUNT).contains(&code) {
        Some((code - PASTE_MARK_BASE) as usize)
    } else {
        None
    }
}

/// `[pasted 4,210 characters]`, the one line shown for a collapsed paste.
pub fn paste_label(chars: usize) -> String {
    let digits = chars.to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("[pasted {grouped} characters]")
}
