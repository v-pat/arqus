//! Draw a `Picker` inside a dialog.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::tui::picker::{Item, Picker, Row};
use crate::tui::theme::Theme;
use crate::tui::widgets::dialog::{centered_rect, draw_dialog};

pub fn draw<T: Clone>(
    frame: &mut Frame,
    area: Rect,
    picker: &mut Picker<T>,
    hint: &str,
    theme: &Theme,
) {
    let (position, total) = picker.position();
    let title = format!("{} · {}/{}", picker.title, position, total);
    let width = area.width.saturating_sub(8).clamp(24, 76);
    let height = area.height.saturating_sub(4).clamp(8, 20);
    let box_area = centered_rect(area, width, height);
    let inner = draw_dialog(frame, box_area, &title, hint, theme);
    if inner.height < 2 {
        return;
    }

    let query = Line::from(vec![
        Span::styled(
            " › ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(picker.query.clone(), Style::default().fg(theme.fg)),
    ]);
    frame.render_widget(Paragraph::new(query), row_area(inner, 0));

    let list_height = inner.height.saturating_sub(2) as usize;
    if list_height == 0 {
        return;
    }
    let rows = picker.window(list_height);
    let selected = picker.selected_index();
    let mut lines: Vec<Line> = Vec::new();
    for row in rows {
        match row {
            Row::Header(text) => lines.push(
                Line::from(format!("  {text}"))
                    .style(Style::default().fg(theme.dim).add_modifier(Modifier::BOLD)),
            ),
            Row::Item { index, positions } => {
                if let Some(item) = picker.items.get(index) {
                    let is_selected = selected == Some(index);
                    lines.push(item_line(
                        item,
                        &positions,
                        is_selected,
                        width as usize,
                        theme,
                    ));
                }
            }
        }
    }
    if lines.is_empty() {
        lines.push(Line::from("  no matches").style(Style::default().fg(theme.dim)));
    }
    let list = Rect {
        y: inner.y + 1,
        height: list_height as u16,
        ..inner
    };
    frame.render_widget(Paragraph::new(Text::from(lines)), list);

    let status = picker.status.clone().unwrap_or_else(|| hint.to_string());
    let line = Line::from(format!(" {status}")).style(Style::default().fg(theme.dim));
    frame.render_widget(
        Paragraph::new(line),
        row_area(inner, inner.height.saturating_sub(1)),
    );
}

fn row_area(area: Rect, offset: u16) -> Rect {
    Rect {
        y: area.y.saturating_add(offset),
        height: 1,
        ..area
    }
}

fn item_line<T: Clone>(
    item: &Item<T>,
    positions: &[usize],
    selected: bool,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let style_for = |fg| {
        let style = Style::default().fg(fg);
        if selected {
            style.bg(theme.selection)
        } else {
            style
        }
    };
    let check = if item.current { "✓ " } else { "  " };
    let prefix = format!(" {check}");
    let mut spans = vec![Span::styled(prefix.clone(), style_for(theme.success))];
    for (index, ch) in item.label.chars().enumerate() {
        let fg = if positions.contains(&index) {
            theme.accent
        } else {
            theme.fg
        };
        spans.push(Span::styled(ch.to_string(), style_for(fg)));
    }
    let left_width = prefix.width() + item.label.width();
    let detail_width = item.detail.width();
    let pad = width.saturating_sub(left_width + detail_width + 1);
    if !item.detail.is_empty() {
        spans.push(Span::styled(" ".repeat(pad), style_for(theme.fg)));
        spans.push(Span::styled(item.detail.clone(), style_for(theme.dim)));
    } else if selected {
        spans.push(Span::styled(" ".repeat(pad), style_for(theme.fg)));
    }
    let mut line = Line::from(spans);
    if selected {
        line = line.style(Style::default().bg(theme.selection));
    }
    line
}
