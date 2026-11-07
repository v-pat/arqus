//! Transcript wrapping. Pure functions, no terminal or ratatui types.

use unicode_width::UnicodeWidthChar;

use super::app::{display_text, UiLine};

/// Display width of one character. Tabs count as four columns.
pub fn char_width(ch: char) -> usize {
    if ch == '\t' {
        4
    } else {
        ch.width().unwrap_or(0)
    }
}

pub fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// Break `text` into lines no wider than `width` display columns.
///
/// Breaks at spaces when possible and hard-breaks words longer than the
/// width. Leading spaces of the input are repeated on continuation lines.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if text.is_empty() {
        return vec![String::new()];
    }
    let expanded = text.replace('\t', "    ");
    let indent: String = expanded.chars().take_while(|c| *c == ' ').collect();
    let content = expanded.trim_start_matches(' ');
    if content.is_empty() {
        return vec![String::new()];
    }
    wrap_indented(content, &indent, width)
}

fn wrap_indented(content: &str, indent: &str, width: usize) -> Vec<String> {
    let avail = width.saturating_sub(display_width(indent)).max(1);
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut line_width = 0;

    for word in content.split(' ').filter(|w| !w.is_empty()) {
        let word_width = display_width(word);
        if line_width > 0 {
            if line_width + 1 + word_width <= avail {
                line.push(' ');
                line.push_str(word);
                line_width += 1 + word_width;
                continue;
            }
            out.push(format!("{indent}{line}"));
            line.clear();
            line_width = 0;
        }
        if word_width <= avail {
            line.push_str(word);
            line_width = word_width;
            continue;
        }
        for ch in word.chars() {
            let w = char_width(ch);
            if line_width + w > avail && line_width > 0 {
                out.push(format!("{indent}{line}"));
                line.clear();
                line_width = 0;
            }
            line.push(ch);
            line_width += w;
        }
    }

    if line_width > 0 || out.is_empty() {
        out.push(format!("{indent}{line}"));
    }
    out
}

/// Wrap every logical line. Lines inside triple backticks are marked dim.
pub fn build_wrapped(lines: &[UiLine], width: usize) -> Vec<UiLine> {
    let width = if width == 0 { usize::MAX } else { width };
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in lines {
        let marker = line.text.trim_start().starts_with("```");
        let dim = in_fence && !marker;
        if marker {
            in_fence = !in_fence;
        }
        let text = display_text(line);
        for part in wrap_text(&text, width) {
            out.push(UiLine {
                kind: line.kind,
                text: part,
                dim,
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "wrap_tests.rs"]
mod tests;
