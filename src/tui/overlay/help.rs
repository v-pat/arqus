//! The `?` shortcuts dialog: two columns generated from the key map and the
//! command table, never hand-written.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::tui::commands::Command;
use crate::tui::keys::{Action, KeyMap};
use crate::tui::theme::Theme;
use crate::tui::widgets::dialog::{centered_rect, draw_dialog};

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme, keymap: &KeyMap, commands: &[Command]) {
    let keys = key_rows(keymap);
    let commands = command_rows(commands);
    let rows = keys.len().div_ceil(2).max(commands.len().saturating_add(1));

    let width = area.width.saturating_sub(4).clamp(40, 78);
    let height = (rows as u16 + 4).min(area.height.saturating_sub(2)).max(8);
    let box_area = centered_rect(area, width, height);
    let inner = draw_dialog(frame, box_area, "Shortcuts", "esc close", theme);
    if inner.height < 2 {
        return;
    }

    let columns = Layout::horizontal([
        Constraint::Percentage(58),
        Constraint::Length(1),
        Constraint::Min(18),
    ])
    .split(inner);
    let key_columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[0]);

    draw_column(
        frame,
        key_columns[0],
        "Keys",
        &keys[..keys.len().div_ceil(2)],
        theme,
    );
    draw_column(
        frame,
        key_columns[1],
        "",
        &keys[keys.len().div_ceil(2)..],
        theme,
    );
    draw_column(frame, columns[2], "Commands", &commands, theme);
}

fn key_rows(keymap: &KeyMap) -> Vec<(String, String)> {
    keymap
        .bindings()
        .into_iter()
        .map(|(key, action)| {
            let description = match action {
                Action::InputChar(_) => action.label().to_string(),
                other => other.short().to_string(),
            };
            (key.label(), description)
        })
        .collect()
}

fn command_rows(commands: &[Command]) -> Vec<(String, String)> {
    commands
        .iter()
        .filter(|command| command.slash && command.action.available())
        .map(|command| (command.label(), command.description.clone()))
        .collect()
}

fn draw_column(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    rows: &[(String, String)],
    theme: &Theme,
) {
    if area.width < 4 || area.height == 0 {
        return;
    }
    let mut lines: Vec<Line> = Vec::new();
    if !title.is_empty() {
        lines.push(
            Line::from(format!(" {title}")).style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }
    let budget = (area.width as usize).saturating_sub(1);
    for (label, description) in rows {
        let label_width = label.width();
        let room = budget.saturating_sub(label_width + 1);
        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(
                label.clone(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", truncate(description, room)),
                Style::default().fg(theme.fg),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)), area);
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
