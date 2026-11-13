//! Drawing for the Connect dialog.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::catalog;
use crate::tui::theme::Theme;
use crate::tui::widgets::dialog::{centered_rect, draw_dialog};
use crate::tui::widgets::picker_view;

use super::{provider_name, Connect, KeyStatus, Step};

pub(super) fn draw(connect: &mut Connect, frame: &mut Frame, area: Rect, theme: &Theme) {
    if let Some(heading) = &connect.heading {
        let line = Line::from(heading.clone())
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(
            Paragraph::new(line),
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
        );
    }
    match &mut connect.step {
        Step::PickProvider { picker } => {
            picker_view::draw(
                frame,
                area,
                picker,
                "↑↓ select · enter choose · esc close",
                theme,
            );
        }
        Step::ProviderMenu { picker, .. } => {
            picker_view::draw(
                frame,
                area,
                picker,
                "↑↓ select · enter choose · esc close",
                theme,
            );
        }
        Step::EnterBaseUrl { provider, input } => {
            let name = provider_name(provider);
            let lines = vec![
                Line::from(format!(" {name}")),
                Line::from(""),
                Line::from(format!(" base URL: {input}")),
            ];
            draw_text_dialog(
                frame,
                area,
                theme,
                &format!("Connect · {name}"),
                "enter continue · esc back",
                lines,
            );
        }
        Step::EnterKey {
            provider,
            input,
            status,
            ..
        } => {
            let name = provider_name(provider);
            let info = catalog::find(provider);
            let key_url = info.map(|info| info.key_url).unwrap_or("");
            let masked = "•".repeat(input.chars().count());
            let mut lines = vec![
                Line::from(format!(" {name}")),
                Line::from(""),
                Line::from(format!(" key: {masked}")),
            ];
            if !key_url.is_empty() {
                lines.push(Line::from(format!(" create one: {key_url}")));
            }
            lines.push(Line::from(""));
            lines.push(status_line(status, theme));
            if let Some(notice) = &connect.notice {
                lines.push(
                    Line::from(format!(" {notice}")).style(Style::default().fg(theme.warning)),
                );
            }
            draw_text_dialog(
                frame,
                area,
                theme,
                &format!("Connect · {name}"),
                "enter check · ctrl+o open key page · ctrl+u clear · esc back",
                lines,
            );
        }
        Step::ConfirmSave { error, .. } => {
            let lines = vec![
                Line::from(format!(" {error}")),
                Line::from(""),
                Line::from(" y save the key anyway   n edit it"),
            ];
            draw_text_dialog(
                frame,
                area,
                theme,
                "Connect · network problem",
                "y save · n edit · esc edit",
                lines,
            );
        }
    }
}

fn status_line(status: &KeyStatus, theme: &Theme) -> Line<'static> {
    match status {
        KeyStatus::Idle => Line::from(""),
        KeyStatus::Checking => {
            Line::from(" ⠋ Checking key...").style(Style::default().fg(theme.accent))
        }
        KeyStatus::Ok(info) => {
            let text = match (info.limit_remaining, info.limit) {
                (Some(remaining), _) => format!(" ✓ Key OK · limit remaining ${remaining:.2}"),
                (None, Some(_)) => " ✓ Key OK · no spending limit".to_string(),
                (None, None) => " ✓ Key OK".to_string(),
            };
            Line::from(text).style(Style::default().fg(theme.success))
        }
        KeyStatus::Error(message) => {
            Line::from(format!(" {message}")).style(Style::default().fg(theme.error))
        }
    }
}

fn draw_text_dialog(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    hint: &str,
    lines: Vec<Line<'static>>,
) {
    let width = area.width.saturating_sub(8).clamp(30, 70);
    let height = (lines.len() as u16 + 4).clamp(8, 16);
    let box_area = centered_rect(area, width, height);
    let inner = draw_dialog(frame, box_area, title, hint, theme);
    frame.render_widget(Paragraph::new(lines), inner);
}
