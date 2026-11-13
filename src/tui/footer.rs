//! The one-line footer and the small facts it shows.
//!
//! Everything here is pure: `layout()` takes state and a width and returns the
//! pieces to draw, so the drop order can be tested without a terminal.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use super::app::App;
use super::facts::{
    cost_text, ctx_level, ctx_percent, fit_path, short_path, truncate_end, CtxLevel,
};
pub use super::facts::{Mode, ProviderState, Tone};
use super::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FooterLayout {
    pub left: Vec<Piece>,
    pub right: Vec<Piece>,
}

fn model_text(state: &ProviderState) -> String {
    match state {
        ProviderState::Connected { provider, model } => format!("{provider}/{model}"),
        ProviderState::NeedsProvider => "no provider".to_string(),
    }
}

fn piece(text: impl Into<String>, tone: Tone) -> Piece {
    Piece {
        text: text.into(),
        tone,
    }
}

fn joined_width(pieces: &[Piece]) -> usize {
    let text: usize = pieces.iter().map(|p| p.text.width()).sum();
    text + pieces.len().saturating_sub(1) * 2
}

#[derive(Debug, Clone, Copy)]
struct Flags {
    hints: bool,
    cost: bool,
    branch: bool,
}

/// Compose the footer, dropping parts in the order hints, cost, branch, path.
pub fn layout(app: &App, width: usize) -> FooterLayout {
    let full_path = short_path(&app.cwd, &app.home);
    let mut path = fit_path(&full_path, width.saturating_sub(2).max(1));
    let mut flags = Flags {
        hints: width >= 100,
        cost: app.session_cost.is_some(),
        branch: app.git.is_some(),
    };
    let avail = width.saturating_sub(2);
    let mut guard = 0;
    loop {
        let total = joined_width(&left_pieces(app, &path, &flags))
            + 2
            + joined_width(&right_pieces(app, &flags));
        if total <= avail || guard > 64 {
            break;
        }
        guard += 1;
        if flags.hints {
            flags.hints = false;
        } else if flags.cost {
            flags.cost = false;
        } else if flags.branch {
            flags.branch = false;
        } else {
            let current = path.width();
            if current <= 10 {
                break;
            }
            path = fit_path(&full_path, current - 1);
        }
    }

    let left = left_pieces(app, &path, &flags);
    let mut right = right_pieces(app, &flags);
    let total = joined_width(&left) + 2 + joined_width(&right);
    if total > avail && !right.is_empty() {
        let rest = joined_width(&right[1..]) + right.len().saturating_sub(2) * 2;
        let budget = avail.saturating_sub(joined_width(&left) + 2 + rest).max(3);
        right[0].text = truncate_end(&right[0].text, budget);
    }
    FooterLayout { left, right }
}

fn left_pieces(app: &App, path: &str, flags: &Flags) -> Vec<Piece> {
    let mut left = vec![piece(path, Tone::Dim)];
    if flags.branch {
        if let Some(git) = &app.git {
            let star = if git.dirty { "*" } else { "" };
            left.push(piece(format!("{}{star}", git.branch), Tone::Accent));
        }
    }
    if flags.hints {
        left.push(piece("? shortcuts", Tone::Dim));
    }
    if !app.follow {
        left.push(piece("scrolled · End to follow", Tone::Warning));
    }
    left
}

fn right_pieces(app: &App, flags: &Flags) -> Vec<Piece> {
    let mut right = vec![piece(model_text(&app.provider_state), Tone::Fg)];
    let percent = ctx_percent(app.context_tokens, app.context_window);
    let tone = match ctx_level(percent) {
        CtxLevel::Ok => Tone::Success,
        CtxLevel::Warn => Tone::Warning,
        CtxLevel::High => Tone::Error,
    };
    right.push(piece(format!("ctx {percent}%"), tone));
    if flags.cost {
        if let Some(cost) = app.session_cost {
            right.push(piece(cost_text(cost), Tone::Dim));
        }
    }
    right.push(piece(app.mode.chip(), Tone::Accent));
    right
}

pub fn tone_style(tone: Tone, theme: &Theme) -> Style {
    let color = match tone {
        Tone::Fg => theme.fg,
        Tone::Dim => theme.dim,
        Tone::Accent => theme.accent,
        Tone::Success => theme.success,
        Tone::Warning => theme.warning,
        Tone::Error => theme.error,
    };
    Style::default().fg(color)
}

pub fn draw(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let footer = layout(app, area.width as usize);
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (index, part) in footer.left.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            part.text.clone(),
            tone_style(part.tone, theme),
        ));
    }
    if !footer.right.is_empty() {
        let left_width = 1 + joined_width(&footer.left);
        let right_width = joined_width(&footer.right);
        let gap = (area.width as usize).saturating_sub(left_width + right_width + 1);
        spans.push(Span::raw(" ".repeat(gap)));
        for (index, part) in footer.right.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                part.text.clone(),
                tone_style(part.tone, theme),
            ));
        }
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
