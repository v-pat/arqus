//! Block rendering: one `Block` to a list of styled ratatui lines.
//!
//! Pure for a given width, theme and the two collapse flags, so the result can
//! be cached per block and inspected in tests.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span as RSpan};

use super::super::block::Block;
use super::super::markdown::{parse_blocks, MdBlock, Span, SpanStyle};
use super::super::msg::Level;
use super::super::styled::wrap_spans;
use super::super::theme::Theme;
use super::super::wrap::wrap_text;
use super::tool_card;

pub fn render_block(
    block: &Block,
    width: usize,
    theme: &Theme,
    expanded: bool,
    show_thinking: bool,
) -> Vec<Line<'static>> {
    let width = width.max(1);
    match block {
        Block::User(text) => user_lines(text, width, theme),
        Block::Assistant { text, .. } => markdown_lines(text, width, theme),
        Block::Thinking { text } => thinking_lines(text, width, theme, show_thinking),
        Block::Tool(tool) => tool_card::tool_lines(tool, width, theme, expanded),
        Block::Notice(text, level) => notice_lines(text, *level, theme),
        Block::Compaction { tokens_before } => vec![Line::from(RSpan::styled(
            format!("[compacted conversation: ~{tokens_before} tokens before]"),
            Style::default().fg(theme.dim).add_modifier(Modifier::DIM),
        ))],
    }
}

fn user_lines(text: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let bar = RSpan::styled(
        "┃ ",
        Style::default().fg(theme.user).add_modifier(Modifier::BOLD),
    );
    wrap_text(text, width.saturating_sub(2).max(1))
        .into_iter()
        .map(|row| {
            Line::from(vec![
                bar.clone(),
                RSpan::styled(row, Style::default().fg(theme.fg)),
            ])
        })
        .collect()
}

fn thinking_lines(
    text: &str,
    width: usize,
    theme: &Theme,
    show_thinking: bool,
) -> Vec<Line<'static>> {
    let dim = Style::default()
        .fg(theme.thinking)
        .add_modifier(Modifier::DIM);
    if !show_thinking {
        return vec![Line::from(RSpan::styled("▸ Thinking... (ctrl+t)", dim))];
    }
    wrap_text(text, width.saturating_sub(2).max(1))
        .into_iter()
        .map(|row| {
            Line::from(vec![
                RSpan::styled("▸ ", dim),
                RSpan::styled(
                    row,
                    Style::default()
                        .fg(theme.thinking)
                        .add_modifier(Modifier::DIM | Modifier::ITALIC),
                ),
            ])
        })
        .collect()
}

fn notice_lines(text: &str, level: Level, theme: &Theme) -> Vec<Line<'static>> {
    let color = match level {
        Level::Info => theme.accent,
        Level::Warn => theme.warning,
        Level::Error => theme.error,
    };
    vec![Line::from(RSpan::styled(
        text.to_string(),
        Style::default().fg(color),
    ))]
}

fn markdown_lines(text: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for block in parse_blocks(text) {
        match block {
            MdBlock::Paragraph(spans) => {
                if spans.is_empty() {
                    out.push(Line::from(""));
                } else {
                    out.extend(paragraph_lines(&spans, width, theme, theme.fg));
                }
            }
            MdBlock::Heading { spans, .. } => {
                let start = out.len();
                out.extend(paragraph_lines(&spans, width, theme, theme.accent));
                for line in &mut out[start..] {
                    for span in &mut line.spans {
                        span.style = span.style.add_modifier(Modifier::BOLD);
                    }
                }
            }
            MdBlock::Bullet(spans) => {
                let start = out.len();
                out.extend(paragraph_lines(
                    &spans,
                    width.saturating_sub(2).max(1),
                    theme,
                    theme.fg,
                ));
                prefix_rows(&mut out, start, "• ", "  ", theme.accent);
            }
            MdBlock::Numbered { marker, spans } => {
                let start = out.len();
                out.extend(paragraph_lines(
                    &spans,
                    width.saturating_sub(2).max(1),
                    theme,
                    theme.fg,
                ));
                let first = format!("{marker} ");
                prefix_rows(&mut out, start, &first, "  ", theme.dim);
            }
            MdBlock::Quote(spans) => {
                let start = out.len();
                out.extend(paragraph_lines(
                    &spans,
                    width.saturating_sub(2).max(1),
                    theme,
                    theme.dim,
                ));
                prefix_rows(&mut out, start, "> ", "> ", theme.dim);
            }
            MdBlock::Code { lines, .. } => {
                for line in lines {
                    for row in wrap_text(&line, width.saturating_sub(2).max(1)) {
                        out.push(Line::from(vec![
                            RSpan::styled("│ ", Style::default().fg(theme.dim)),
                            RSpan::styled(row, code_style(theme)),
                        ]));
                    }
                }
            }
        }
    }
    out
}

fn paragraph_lines(
    spans: &[Span],
    width: usize,
    theme: &Theme,
    color: Color,
) -> Vec<Line<'static>> {
    let rows = wrap_spans(spans, width);
    rows.into_iter()
        .map(|row| {
            let parts: Vec<RSpan> = row
                .iter()
                .map(|span| RSpan::styled(span.text.clone(), inline_style(span, theme, color)))
                .collect();
            Line::from(parts)
        })
        .collect()
}

/// Add a prefix to every row of the block that ended at `out.len()`.
fn prefix_rows(out: &mut [Line<'static>], start: usize, first: &str, rest: &str, color: Color) {
    for (offset, line) in out[start..].iter_mut().enumerate() {
        let label = if offset == 0 { first } else { rest };
        let mut spans = vec![RSpan::styled(label.to_string(), Style::default().fg(color))];
        spans.extend(line.spans.iter().cloned());
        *line = Line::from(spans);
    }
}

fn inline_style(span: &Span, theme: &Theme, color: Color) -> Style {
    let base = Style::default().fg(color);
    match span.style {
        SpanStyle::Plain => base,
        SpanStyle::Bold => base.add_modifier(Modifier::BOLD),
        SpanStyle::Italic => base.add_modifier(Modifier::ITALIC),
        SpanStyle::Code => Style::default().fg(theme.fg).bg(theme.code_bg),
    }
}

fn code_style(theme: &Theme) -> Style {
    Style::default().fg(theme.fg).bg(theme.code_bg)
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
