//! The welcome screen: wordmark, tagline, centered prompt box and hints.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use super::app::App;
use super::facts::provider_line;
use super::footer::tone_style;
use super::theme::Theme;

const WORDMARK: &str = "a r q u s";
const TAGLINE: &str = "a coding agent for your terminal";
const HINTS: &str = "ctrl+p commands    ctrl+l models    ? shortcuts";
const PLACEHOLDER: &str = "▌ Ask anything...   try: \"fix the failing test\"";

pub fn draw(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let top = top_of(area);

    centered(
        frame,
        area,
        top,
        WORDMARK,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    centered(
        frame,
        area,
        top + 1,
        TAGLINE,
        Style::default().fg(theme.dim),
    );

    let box_area = input_box(area, top + 3);
    if app.overlays.is_empty() {
        draw_box(app, frame, box_area, theme);
    }

    let (line, tone) = provider_line(&app.provider_state);
    centered(frame, area, top + 6, &line, tone_style(tone, theme));
    centered(frame, area, top + 8, HINTS, Style::default().fg(theme.dim));
}

fn top_of(area: Rect) -> u16 {
    area.y + area.height.saturating_sub(9) / 2
}

/// Top row of the prompt box, so the `/` menu can sit just above it.
pub fn input_top(area: Rect) -> u16 {
    top_of(area) + 3
}

fn input_box(area: Rect, y: u16) -> Rect {
    let width = 60.min(area.width.saturating_sub(4)).max(20.min(area.width));
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y,
        width,
        height: 3.min(area.bottom().saturating_sub(y)),
    }
}

fn draw_box(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let block = Block::bordered().border_style(Style::default().fg(theme.border_focus));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let text = app.input.text();
    let content = if text.is_empty() {
        truncate(PLACEHOLDER, inner.width as usize)
    } else {
        format!(
            "▌ {}",
            truncate(
                text.lines().next().unwrap_or(""),
                inner.width.saturating_sub(2) as usize
            )
        )
    };
    let style = if text.is_empty() {
        Style::default().fg(theme.dim)
    } else {
        Style::default().fg(theme.fg)
    };
    frame.render_widget(
        Paragraph::new(Line::from(content)).style(style),
        Rect {
            x: inner.x + 1,
            width: inner.width.saturating_sub(1),
            ..inner
        },
    );

    if app.overlays.is_empty()
        && app.resume.is_none()
        && app.tree.is_none()
        && app.approval.is_none()
    {
        let column = if app.input.text().is_empty() {
            1u16
        } else {
            2u16.saturating_add(app.input.cursor_column().min(u16::MAX as usize) as u16)
        };
        let x = (inner.x + 1 + column).min(inner.right().saturating_sub(1));
        frame.set_cursor_position((x, inner.y));
    }
}

fn centered(frame: &mut Frame, area: Rect, y: u16, text: &str, style: Style) {
    if y >= area.bottom() {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(text.to_string()))
            .alignment(Alignment::Center)
            .style(style),
        Rect {
            y,
            height: 1,
            ..area
        },
    );
}

fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let cell = ch.to_string().width();
        if used + cell > width.saturating_sub(1) {
            break;
        }
        used += cell;
        out.push(ch);
    }
    out.push('…');
    out
}
