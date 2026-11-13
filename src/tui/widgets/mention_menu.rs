//! The `@` popup, drawn just above the prompt box.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::app::App;
use crate::tui::picker::Row;
use crate::tui::theme::Theme;

const MAX_ROWS: usize = 6;

pub fn draw(frame: &mut Frame, area: Rect, app: &App, anchor_y: u16, theme: &Theme) {
    let Some(picker) = &app.mention else {
        return;
    };
    let rows = picker.rows();
    if rows.is_empty() {
        return;
    }
    let height = rows.len().min(MAX_ROWS) as u16 + 2;
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
        .title(" files ")
        .border_style(Style::default().fg(theme.border_focus));
    let inner = block.inner(box_area);
    frame.render_widget(block, box_area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let selected = picker.selected_index();
    let selected_row = rows
        .iter()
        .position(|row| matches!(row, Row::Item { index, .. } if Some(*index) == selected))
        .unwrap_or(0);
    let start = selected_row.saturating_sub(MAX_ROWS - 1);

    let mut lines: Vec<Line> = Vec::new();
    for row in rows.iter().skip(start).take(MAX_ROWS) {
        let Row::Item { index, .. } = row else {
            continue;
        };
        let Some(item) = picker.items.get(*index) else {
            continue;
        };
        let is_selected = Some(*index) == selected;
        let marker = if is_selected { " ▸ " } else { "   " };
        let style = if is_selected {
            Style::default().bg(theme.selection)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(
                marker.to_string(),
                style.fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(item.label.clone(), style.fg(theme.fg)),
        ]));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}
