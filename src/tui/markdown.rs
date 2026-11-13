//! A tiny markdown parser for assistant text. Pure: no drawing, no I/O.
//!
//! Only the subset in the stage spec is supported: bold, italic, inline code,
//! fenced code blocks, `#` headings, `-`/`*` bullets, numbered lists and `>`
//! quotes. Tables and links are not special: a URL is plain text.
//!
//! Half-finished input must never panic or eat the text. `**bol` stays plain
//! and an unclosed fence still produces a code block.

use super::styled::display_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStyle {
    Plain,
    Bold,
    Italic,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: SpanStyle,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Span {
        Span {
            text: text.into(),
            style: SpanStyle::Plain,
        }
    }

    pub fn width(&self) -> usize {
        display_width(&self.text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdBlock {
    /// One source line. An empty line is a paragraph with no spans.
    Paragraph(Vec<Span>),
    Heading {
        level: usize,
        spans: Vec<Span>,
    },
    Bullet(Vec<Span>),
    Numbered {
        marker: String,
        spans: Vec<Span>,
    },
    Quote(Vec<Span>),
    Code {
        lang: String,
        lines: Vec<String>,
        closed: bool,
    },
}

/// Parse one line of inline markdown. Unclosed markers stay literal.
pub fn parse_inline(text: &str) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;

    while i < chars.len() {
        let rest = &chars[i..];
        if rest.starts_with(&['*', '*']) {
            if let Some(end) = find(&chars, i + 2, &['*', '*']) {
                if end > i + 2 {
                    flush(&mut plain, &mut spans);
                    spans.push(Span {
                        text: chars[i + 2..end].iter().collect(),
                        style: SpanStyle::Bold,
                    });
                    i = end + 2;
                    continue;
                }
            }
            plain.push('*');
            i += 1;
            continue;
        }
        if rest[0] == '`' {
            if let Some(end) = find(&chars, i + 1, &['`']) {
                if end > i + 1 {
                    flush(&mut plain, &mut spans);
                    spans.push(Span {
                        text: chars[i + 1..end].iter().collect(),
                        style: SpanStyle::Code,
                    });
                    i = end + 1;
                    continue;
                }
            }
            plain.push('`');
            i += 1;
            continue;
        }
        if rest[0] == '*' {
            if let Some(end) = find(&chars, i + 1, &['*']) {
                if end > i + 1 {
                    flush(&mut plain, &mut spans);
                    spans.push(Span {
                        text: chars[i + 1..end].iter().collect(),
                        style: SpanStyle::Italic,
                    });
                    i = end + 1;
                    continue;
                }
            }
            plain.push('*');
            i += 1;
            continue;
        }
        plain.push(rest[0]);
        i += 1;
    }
    flush(&mut plain, &mut spans);
    spans
}

fn flush(plain: &mut String, spans: &mut Vec<Span>) {
    if !plain.is_empty() {
        spans.push(Span::plain(std::mem::take(plain)));
    }
}

/// Find the first occurrence of `needle` at or after `from`.
fn find(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    if needle.is_empty() || chars.len() < needle.len() {
        return None;
    }
    let last = chars.len() - needle.len();
    (from..=last).find(|start| {
        needle
            .iter()
            .enumerate()
            .all(|(offset, ch)| chars[*start + offset] == *ch)
    })
}

/// Split assistant text into markdown blocks, one block per source line
/// (except fenced code, which groups lines).
pub fn parse_blocks(text: &str) -> Vec<MdBlock> {
    let mut out = Vec::new();
    let mut code: Option<(String, Vec<String>)> = None;

    for line in text.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            match code.take() {
                Some((lang, lines)) => out.push(MdBlock::Code {
                    lang,
                    lines,
                    closed: true,
                }),
                None => {
                    let lang = trimmed.trim_start_matches('`').trim().to_string();
                    code = Some((lang, Vec::new()));
                }
            }
            continue;
        }
        if let Some((_, lines)) = code.as_mut() {
            lines.push(line.to_string());
            continue;
        }
        out.push(parse_line(line));
    }
    if let Some((lang, lines)) = code.take() {
        // An unclosed fence while streaming still counts as a code block.
        out.push(MdBlock::Code {
            lang,
            lines,
            closed: false,
        });
    }
    out
}

fn parse_line(line: &str) -> MdBlock {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return MdBlock::Paragraph(Vec::new());
    }
    if let Some((level, body)) = heading(trimmed) {
        return MdBlock::Heading {
            level,
            spans: parse_inline(body),
        };
    }
    if let Some(body) = bullet(trimmed) {
        return MdBlock::Bullet(parse_inline(body));
    }
    if let Some((marker, body)) = numbered(trimmed) {
        return MdBlock::Numbered {
            marker,
            spans: parse_inline(body),
        };
    }
    if let Some(body) = trimmed.strip_prefix('>') {
        return MdBlock::Quote(parse_inline(body.trim_start()));
    }
    MdBlock::Paragraph(parse_inline(trimmed))
}

fn heading(text: &str) -> Option<(usize, &str)> {
    let hashes = text.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &text[hashes..];
    if rest.is_empty() {
        return Some((hashes, ""));
    }
    rest.strip_prefix(' ').map(|body| (hashes, body))
}

fn bullet(text: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(body) = text.strip_prefix(marker) {
            return Some(body);
        }
    }
    None
}

fn numbered(text: &str) -> Option<(String, &str)> {
    let digits = text.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let rest = &text[digits..];
    let mut chars = rest.chars();
    let marker = chars.next()?;
    if marker != '.' && marker != ')' {
        return None;
    }
    let body = chars.as_str().strip_prefix(' ')?;
    Some((format!("{}{}", &text[..digits], marker), body))
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
