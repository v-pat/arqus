//! Drawing only. All state lives in `App`; caches may be refreshed here.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::app::App;
use super::theme::Theme;
use super::widgets::dialog::centered_rect;
use super::widgets::mention_menu;
use super::widgets::picker_view;
use super::widgets::slash_menu;
use super::wrap::wrap_text;
use super::{footer, running, toast, welcome};

pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 15;

/// Editor border follows the mode: ask = border, auto = warning, plan = accent.
pub fn border_color(mode: footer::Mode, theme: &Theme) -> ratatui::style::Color {
    match mode {
        footer::Mode::Ask => theme.border,
        footer::Mode::Auto => theme.warning,
        footer::Mode::Plan => theme.accent,
    }
}

pub fn view(app: &mut App, frame: &mut Frame) {
    let theme = app.theme.clone();
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let line =
            Line::from("terminal too small (need 60x15)").style(Style::default().fg(theme.dim));
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(area);
    if app.has_conversation() {
        draw_chat(app, frame, rows[0], &theme);
    } else {
        welcome::draw(app, frame, rows[0], &theme);
    }
    let anchor = if app.has_conversation() {
        rows[0].bottom().saturating_sub(3)
    } else {
        welcome::input_top(rows[0])
    };
    slash_menu::draw(frame, rows[0], app, anchor, &theme);
    mention_menu::draw(frame, rows[0], app, anchor, &theme);
    footer::draw(frame, app, rows[1], &theme);
    toast::draw(frame, app, area, &theme);

    if let Some(resume) = &mut app.resume {
        let hint = if resume.confirm.is_some() {
            "y move to trash · n keep · esc keep"
        } else {
            "↑↓ select · enter open · ctrl+d delete · esc cancel"
        };
        picker_view::draw(frame, area, &mut resume.picker, hint, &theme);
    }
    if let Some(tree) = &mut app.tree {
        picker_view::draw(
            frame,
            area,
            &mut tree.picker,
            "↑↓ select · enter switch · esc cancel",
            &theme,
        );
    }
    for index in 0..app.overlays.len() {
        app.overlays[index].draw(frame, area, &theme, &app.keymap, &app.commands);
    }
    // The approval dialog is always on top and owns every key.
    if let Some(approval) = &app.approval {
        draw_approval(frame, approval, area, &theme);
    }
}

fn draw_chat(app: &mut App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let (view_start, view_end) = app.input.view(6);
    let input_rows = (view_end - view_start).max(1) as u16;
    let queue_rows = app.queue.len().min(3) as u16;
    let running_rows = if app.running { 1 } else { 0 };
    let status_rows = if app.status == "ready" || app.status.is_empty() {
        0
    } else {
        1
    };
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(running_rows),
        Constraint::Length(status_rows),
        Constraint::Length(queue_rows),
        Constraint::Length(input_rows + 2),
    ])
    .split(area);

    draw_transcript(frame, app, chunks[0]);
    if running_rows == 1 {
        draw_running(frame, app, chunks[1], theme);
    }
    if status_rows == 1 {
        draw_status(frame, app, chunks[2], theme);
    }
    if queue_rows > 0 {
        draw_queue(frame, app, chunks[3], theme);
    }
    if app.overlays.is_empty() {
        draw_input(frame, app, chunks[4], theme);
    }
}

fn draw_queue(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let hidden = app.queue.len().saturating_sub(2);
    let mut items: Vec<Line> = app
        .queue
        .iter()
        .take(2)
        .map(|text| Line::from(format!(" queued: {text}")))
        .collect();
    if hidden > 0 {
        items.push(Line::from(format!(" ... {hidden} more queued")));
    }
    let style = Style::default().fg(theme.dim).add_modifier(Modifier::DIM);
    frame.render_widget(Paragraph::new(Text::from(items)).style(style), area);
}

fn draw_transcript(frame: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || app.wrapped.is_empty() {
        return;
    }
    // Only the visible rows are cloned; 5,000 blocks stay cheap to draw.
    let start = app.scroll.min(app.wrapped.len().saturating_sub(1));
    let end = (start + area.height as usize).min(app.wrapped.len());
    let lines: Vec<Line> = app.wrapped[start..end].to_vec();
    frame.render_widget(Paragraph::new(Text::from(lines)), area);
}

fn draw_running(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let Some(text) = running::line(app) else {
        return;
    };
    let color = if app.retry_notice.is_some() {
        theme.warning
    } else {
        theme.accent
    };
    let line = Line::from(format!(" {text}")).style(Style::default().fg(color));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let line = Line::from(format!(" {}", app.status)).style(Style::default().fg(theme.dim));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let border = border_color(app.mode, theme);
    let block = Block::bordered()
        .title(" prompt ")
        .border_style(Style::default().fg(border));
    let (start, end) = app.input.view(6);
    let lines: Vec<Line> = (start..end)
        .map(|line| {
            let mut spans: Vec<Span> = Vec::new();
            for part in app.input.line_parts(line) {
                match part {
                    super::input::LinePart::Text(text) => {
                        spans.push(Span::styled(text, Style::default().fg(theme.fg)));
                    }
                    super::input::LinePart::Paste(chars) => {
                        spans.push(Span::styled(
                            super::input::paste_label(chars),
                            Style::default().fg(theme.dim).add_modifier(Modifier::DIM),
                        ));
                    }
                }
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);

    if app.resume.is_none() && app.tree.is_none() && app.overlays.is_empty() {
        let column = app
            .input
            .cursor_column()
            .min(area.width.saturating_sub(3) as usize) as u16;
        let row = app.input.cursor_line().saturating_sub(start) as u16;
        let x = (area.x + 1 + column).min(area.right().saturating_sub(2));
        let y = (area.y + 1 + row).min(area.bottom().saturating_sub(2));
        frame.set_cursor_position((x, y));
    }
}

fn draw_approval(
    frame: &mut Frame,
    approval: &super::approval::Approval,
    area: Rect,
    theme: &Theme,
) {
    let width = area.width.saturating_sub(8).clamp(40, 78);
    let inner_width = width.saturating_sub(2) as usize;
    let room = area.height.saturating_sub(6) as usize;
    let max_lines = match approval.tool.as_str() {
        "bash" => 10,
        _ => 20,
    }
    .min(room.max(1));

    let mut body: Vec<(String, bool)> = Vec::new();
    for line in approval.preview.lines() {
        if line == "RISKY" {
            body.push((line.to_string(), true));
            continue;
        }
        for wrapped in wrap_text(line, inner_width) {
            body.push((wrapped, false));
        }
    }
    let hidden = body.len().saturating_sub(max_lines);
    if hidden > 0 {
        body.truncate(max_lines.saturating_sub(1));
        body.push((format!("… +{hidden} more lines"), false));
    }

    let height = (body.len() + 4).min(area.height.saturating_sub(2) as usize) as u16;
    let box_area = centered_rect(area, width, height);
    frame.render_widget(ratatui::widgets::Clear, box_area);

    let mut items: Vec<Line> = body
        .into_iter()
        .map(|(text, risky)| {
            let style = if risky {
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };
            Line::from(format!(" {text}")).style(style)
        })
        .collect();
    items.push(Line::from(""));
    items.push(
        Line::from(format!(
            " y allow once · a always allow {} · n deny · esc stop run ",
            approval.tool
        ))
        .style(Style::default().fg(theme.dim)),
    );
    let title = match approval.tool.as_str() {
        "bash" => "Run command?".to_string(),
        "write" => "Write file?".to_string(),
        "edit" => "Edit file?".to_string(),
        other => format!("Approve {other}?"),
    };
    let modal = Paragraph::new(Text::from(items)).block(
        Block::bordered()
            .title(format!(" {title} "))
            .border_style(Style::default().fg(theme.warning)),
    );
    frame.render_widget(modal, box_area);
}
