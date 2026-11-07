//! Drawing only. All state lives in `App`; caches may be refreshed here.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use super::app::{App, LineKind, UiLine};
use super::theme::Theme;
use super::widgets::dialog::centered_rect;
use super::widgets::picker_view;

pub fn view(app: &mut App, frame: &mut Frame) {
    let theme = app.theme.clone();
    let area = frame.area();
    let (view_start, view_end) = app.input.view(6);
    let input_rows = (view_end - view_start).max(1) as u16;
    let hint_rows = if app.input.hint().is_some() { 1 } else { 0 };
    let queue_rows = app.queue.len().min(3) as u16;
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(queue_rows),
        Constraint::Length(input_rows + 2),
        Constraint::Length(hint_rows),
    ])
    .split(area);

    draw_transcript(frame, app, chunks[0], &theme);
    draw_status(frame, app, chunks[1], &theme);
    if queue_rows > 0 {
        draw_queue(frame, app, chunks[2], &theme);
    }
    draw_input(frame, app, chunks[3], &theme);
    if hint_rows == 1 {
        draw_hint(frame, app, chunks[4], &theme);
    }
    if let Some(resume) = &mut app.resume {
        picker_view::draw(
            frame,
            area,
            &mut resume.picker,
            "↑↓ select · enter open · esc cancel",
            &theme,
        );
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
    if let Some(approval) = &app.approval {
        draw_approval(frame, approval, area, &theme);
    }
    for overlay in &app.overlays {
        overlay.draw(frame, area, &theme);
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

fn draw_transcript(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let lines: Vec<Line> = app
        .wrapped
        .iter()
        .map(|line| to_line(line, theme))
        .collect();
    let scroll = app.scroll.min(u16::MAX as usize) as u16;
    let paragraph = Paragraph::new(Text::from(lines)).scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let percent = app
        .context_tokens
        .saturating_mul(100)
        .checked_div(app.context_window)
        .map(|p| p.min(100))
        .unwrap_or(0);
    let file = if app.session_file.is_empty() {
        "no session"
    } else {
        &app.session_file
    };
    let cost = match app.session_cost {
        Some(cost) => format!("cost=${cost:.4}"),
        None => "cost=unknown".to_string(),
    };

    let mut parts = vec![
        app.model.clone(),
        format!(
            "{}% ({}/{})",
            percent, app.context_tokens, app.context_window
        ),
        file.to_string(),
        cost,
    ];
    if app.running {
        parts.push(format!("{} {}s", app.spinner(), app.elapsed_secs()));
    }
    if !app.follow {
        parts.push("[scrolled - End to follow]".to_string());
    }
    parts.push(app.status.clone());

    let line = Line::from(parts.join(" | ")).style(Style::default().fg(theme.dim));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::bordered()
        .title(" prompt ")
        .border_style(Style::default().fg(theme.border));
    let (start, end) = app.input.view(6);
    let lines: Vec<Line> = (start..end)
        .map(|line| Line::from(app.input.line_text(line)).style(Style::default().fg(theme.fg)))
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

fn draw_hint(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let Some(hint) = app.input.hint() else {
        return;
    };
    let line = Line::from(format!(" {hint}")).style(Style::default().fg(theme.dim));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_approval(
    frame: &mut Frame,
    approval: &super::approval::Approval,
    area: Rect,
    theme: &Theme,
) {
    let box_area = centered_rect(
        area,
        area.width.saturating_sub(8),
        area.height.saturating_sub(4),
    );
    frame.render_widget(ratatui::widgets::Clear, box_area);
    let rows = box_area.height.saturating_sub(2) as usize;
    let mut items: Vec<Line> = Vec::new();
    for line in approval.preview.lines().take(rows.saturating_sub(2)) {
        let style = if line == "RISKY" {
            Style::default()
                .fg(theme.error)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        items.push(Line::from(line.to_string()).style(style));
    }
    items.push(Line::from(""));
    items.push(Line::from(format!(
        " y allow once   a allow {} for this session   n deny   Esc deny and abort ",
        approval.tool
    )));
    let title = format!(" approve {} ", approval.tool);
    let modal = Paragraph::new(Text::from(items)).block(
        Block::bordered()
            .title(title)
            .border_style(Style::default().fg(theme.warning)),
    );
    frame.render_widget(modal, box_area);
}

fn to_line(line: &UiLine, theme: &Theme) -> Line<'static> {
    let mut style = match line.kind {
        LineKind::User => Style::default().fg(theme.user).add_modifier(Modifier::BOLD),
        LineKind::Assistant => Style::default().fg(theme.assistant),
        LineKind::Thinking => Style::default()
            .fg(theme.thinking)
            .add_modifier(Modifier::ITALIC),
        LineKind::ToolCall => Style::default().fg(theme.tool),
        LineKind::ToolResult => Style::default().fg(theme.fg),
        LineKind::Error => Style::default().fg(theme.error),
        LineKind::Info => Style::default().fg(theme.accent),
    };
    if line.dim {
        style = style.fg(theme.dim).add_modifier(Modifier::DIM);
    }
    Line::from(line.text.clone()).style(style)
}
