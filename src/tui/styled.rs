//! Display-width helpers and span wrapping, shared by markdown and blocks.

use unicode_width::UnicodeWidthChar;

use super::markdown::{Span, SpanStyle};

pub fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| {
            if ch == '\t' {
                4
            } else {
                ch.width().unwrap_or(0)
            }
        })
        .sum()
}

fn char_width(ch: char) -> usize {
    if ch == '\t' {
        4
    } else {
        ch.width().unwrap_or(0)
    }
}

/// Wrap styled spans to `width` display columns. Breaks at spaces, hard-breaks
/// words that are longer than the width, and never returns zero rows.
pub fn wrap_spans(spans: &[Span], width: usize) -> Vec<Vec<Span>> {
    let width = width.max(1);
    let mut rows: Vec<Vec<(char, SpanStyle)>> = Vec::new();
    let mut row: Vec<(char, SpanStyle)> = Vec::new();
    let mut row_width = 0;
    let mut last_space: Option<usize> = None;

    for span in spans {
        for ch in span.text.chars() {
            if ch == '\n' {
                rows.push(std::mem::take(&mut row));
                row_width = 0;
                last_space = None;
                continue;
            }
            let w = char_width(ch);
            if row_width + w > width && !row.is_empty() {
                if let Some(space) = last_space {
                    let tail: Vec<(char, SpanStyle)> = row.split_off(space + 1);
                    row.pop();
                    rows.push(std::mem::take(&mut row));
                    row_width = tail_width(&tail);
                    last_space = tail.iter().position(|(c, _)| *c == ' ');
                    row = tail;
                } else {
                    rows.push(std::mem::take(&mut row));
                    row_width = 0;
                    last_space = None;
                }
            }
            if ch == ' ' {
                last_space = Some(row.len());
            }
            row.push((ch, span.style));
            row_width += w;
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows.into_iter()
        .map(|chars| chars_to_spans(&chars))
        .collect()
}

fn tail_width(chars: &[(char, SpanStyle)]) -> usize {
    chars.iter().map(|(ch, _)| char_width(*ch)).sum()
}

fn chars_to_spans(chars: &[(char, SpanStyle)]) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    for (ch, style) in chars {
        match spans.last_mut() {
            Some(last) if last.style == *style => last.text.push(*ch),
            _ => spans.push(Span {
                text: ch.to_string(),
                style: *style,
            }),
        }
    }
    spans
}

#[cfg(test)]
#[path = "styled_tests.rs"]
mod tests;
