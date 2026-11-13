//! Transient messages drawn at the top right.
//!
//! Time is measured in 100 ms ticks, the same clock as the spinner, so the
//! expiry logic is pure and testable.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use super::app::App;
use super::msg::Level;
use super::theme::Theme;

/// 3 seconds for notices, 6 for errors.
const INFO_TICKS: u64 = 30;
const ERROR_TICKS: u64 = 60;
pub const MAX_VISIBLE: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub level: Level,
    /// Tick at which the toast disappears.
    pub expires_at: u64,
}

impl Toast {
    pub fn new(text: impl Into<String>, level: Level, now: u64) -> Toast {
        let ttl = if level == Level::Error {
            ERROR_TICKS
        } else {
            INFO_TICKS
        };
        Toast {
            text: text.into(),
            level,
            expires_at: now.saturating_add(ttl),
        }
    }
}

impl App {
    /// Add a toast and mirror the text in the status line.
    pub fn toast(&mut self, text: impl Into<String>, level: Level) {
        let text = text.into();
        self.status = text.clone();
        self.toasts.push_back(Toast::new(text, level, self.ticks));
        while self.toasts.len() > MAX_VISIBLE {
            self.toasts.pop_front();
        }
        self.dirty = true;
    }

    /// Drop every toast whose time is up.
    pub fn prune_toasts(&mut self) {
        if self.toasts.is_empty() {
            return;
        }
        let now = self.ticks;
        let before = self.toasts.len();
        self.toasts.retain(|toast| toast.expires_at > now);
        if self.toasts.len() != before {
            self.dirty = true;
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if app.toasts.is_empty() || area.width < 4 || area.height == 0 {
        return;
    }
    let visible: Vec<&Toast> = app.toasts.iter().rev().take(MAX_VISIBLE).collect();
    let widest = visible
        .iter()
        .map(|toast| toast.text.width())
        .max()
        .unwrap_or(0);
    let width = (widest as u16 + 2).min(area.width);
    let height = (visible.len() as u16).min(area.height);
    let rect = Rect {
        x: area.right().saturating_sub(width),
        y: area.y,
        width,
        height,
    };
    let lines: Vec<Line> = visible
        .iter()
        .map(|toast| {
            let color = match toast.level {
                Level::Info => theme.accent,
                Level::Warn => theme.warning,
                Level::Error => theme.error,
            };
            Line::from(format!("{} ", toast.text)).style(Style::default().fg(color))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(Text::from(lines)).alignment(Alignment::Right),
        rect,
    );
}

#[cfg(test)]
#[path = "toast_tests.rs"]
mod tests;
