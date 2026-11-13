//! The `/` popup, drawn above the prompt box.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::tui::app::App;
use crate::tui::facts::truncate_end;
use crate::tui::theme::Theme;

pub fn draw(frame: &mut Frame, area: Rect, app: &App, anchor_y: u16, theme: &Theme) {
    if !app.slash.open {
        return;
    }
    let rows = app.slash.matched.len().clamp(1, 8) as u16;
    let height = rows + 2;
    if area.width < 20 || anchor_y < area.y + height {
        return;
    }
    let width = area.width.saturating_sub(4).clamp(20, 72);
    let box_area = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: anchor_y - height,
        width,
        height,
    };
    frame.render_widget(Clear, box_area);
    let block = Block::bordered()
        .title(" commands ")
        .border_style(Style::default().fg(theme.border_focus));
    let inner = block.inner(box_area);
    frame.render_widget(block, box_area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    if app.slash.matched.is_empty() {
        lines.push(Line::from("  no matches").style(Style::default().fg(theme.dim)));
    }
    for (row, index) in app.slash.matched.iter().enumerate() {
        let Some(command) = app.commands.get(*index) else {
            continue;
        };
        let selected = row == app.slash.selected;
        let marker = if selected { " ▸ " } else { "   " };
        let label = format!("/{}", command.name);
        let hint = command.key_hint.unwrap_or("");
        let used = marker.width() + label.width() + 2 + hint.width();
        let room = (inner.width as usize).saturating_sub(used + 2);
        let description = truncate_end(&command.description, room);
        let style = if selected {
            Style::default().bg(theme.selection)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(
                marker.to_string(),
                style.fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(label, style.fg(theme.fg)),
            Span::styled(format!("  {description}"), style.fg(theme.dim)),
            Span::styled(format!("  {hint}"), style.fg(theme.dim)),
        ]));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}
