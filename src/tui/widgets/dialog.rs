//! Shared dialog frame: clear, border, title and a dim hint line.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// A box of `width` x `height` centered in `area`, clamped to the area.
pub fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Draw a bordered dialog and its hint. Returns the area inside the border,
/// minus the hint row.
pub fn draw_dialog(frame: &mut Frame, area: Rect, title: &str, hint: &str, theme: &Theme) -> Rect {
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(format!(" {title} "))
        .border_style(Style::default().fg(theme.border_focus));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return inner;
    }
    let hint_area = Rect {
        y: inner.y + inner.height - 1,
        height: 1,
        ..inner
    };
    let line = Line::from(format!(" {hint}"))
        .style(Style::default().fg(theme.dim).add_modifier(Modifier::DIM));
    frame.render_widget(Paragraph::new(line), hint_area);
    Rect {
        height: inner.height - 1,
        ..inner
    }
}
