//! Drawing only. All state lives in `App`.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Clear, List, Paragraph};
use ratatui::Frame;

use super::app::{App, LineKind, UiLine};
use super::layout::visible_window;

pub fn draw(frame: &mut Frame, app: &App) {
    let (start, end) = app.input.view(6);
    let input_rows = (end - start).max(1) as u16;
    let hint_rows = if app.input.hint().is_some() { 1 } else { 0 };
    let queue_rows = app.queue.len().min(3) as u16;
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(queue_rows),
        Constraint::Length(input_rows + 2),
        Constraint::Length(hint_rows),
    ])
    .split(frame.area());

    draw_transcript(frame, app, chunks[0]);
    draw_status(frame, app, chunks[1]);
    if queue_rows > 0 {
        draw_queue(frame, app, chunks[2]);
    }
    draw_input(frame, app, chunks[3]);
    if hint_rows == 1 {
        draw_hint(frame, app, chunks[4]);
    }
    if app.resume.is_some() {
        draw_resume(frame, app, frame.area());
    }
    if app.tree.is_some() {
        draw_tree(frame, app, frame.area());
    }
    if app.approval.is_some() {
        draw_approval(frame, app, frame.area());
    }
}

/// Prompts typed during a run, dim, just above the prompt box.
fn draw_queue(frame: &mut Frame, app: &App, area: Rect) {
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
    let style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM);
    frame.render_widget(Paragraph::new(Text::from(items)).style(style), area);
}

fn draw_transcript(frame: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = app.wrapped.iter().map(to_line).collect();
    let scroll = app.scroll.min(u16::MAX as usize) as u16;
    let paragraph = Paragraph::new(Text::from(lines)).scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
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

    let line = Line::from(parts.join(" | ")).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered().title(" prompt ");
    let (start, end) = app.input.view(6);
    let lines: Vec<Line> = (start..end)
        .map(|line| Line::from(app.input.line_text(line)))
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);

    if app.resume.is_none() && app.tree.is_none() {
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

fn draw_hint(frame: &mut Frame, app: &App, area: Rect) {
    let Some(hint) = app.input.hint() else {
        return;
    };
    let line = Line::from(format!(" {hint}")).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_resume(frame: &mut Frame, app: &App, area: Rect) {
    let Some(resume) = &app.resume else {
        return;
    };
    let box_area = centered(area, 70, 60);
    frame.render_widget(Clear, box_area);

    let rows = box_area.height.saturating_sub(2) as usize;
    let (start, end) = visible_window(resume.selected, resume.paths.len(), rows);
    let items: Vec<Line> = (start..end)
        .map(|i| {
            let path = &resume.paths[i];
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let time = resume.times.get(i).map(String::as_str).unwrap_or("");
            let style = if i == resume.selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(format!(" {name}  {time}")).style(style)
        })
        .collect();

    let title = format!(
        " resume {}/{}: Up/Down, Enter open, Esc cancel ",
        resume.selected + 1,
        resume.paths.len()
    );
    let list = List::new(items).block(Block::bordered().title(title));
    frame.render_widget(list, box_area);
}

fn draw_tree(frame: &mut Frame, app: &App, area: Rect) {
    let Some(state) = &app.tree else {
        return;
    };
    let box_area = centered(area, 80, 70);
    frame.render_widget(Clear, box_area);

    let rows = box_area.height.saturating_sub(2) as usize;
    let (start, end) = visible_window(state.selected, state.rows.len(), rows);
    let items: Vec<Line> = (start..end)
        .map(|i| {
            let row = &state.rows[i];
            let marker = if row.on_path { "*" } else { " " };
            let indent = "  ".repeat(row.depth);
            let style = if i == state.selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if row.on_path {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            Line::from(format!(" {marker} {indent}{}: {}", row.role, row.preview)).style(style)
        })
        .collect();

    let title = format!(
        " tree {}/{}: Up/Down, Enter switch, Esc cancel ",
        state.selected + 1,
        state.rows.len()
    );
    let list = List::new(items).block(Block::bordered().title(title));
    frame.render_widget(list, box_area);
}

fn draw_approval(frame: &mut Frame, app: &App, area: Rect) {
    let Some(approval) = &app.approval else {
        return;
    };
    let box_area = centered(area, 80, 70);
    frame.render_widget(Clear, box_area);
    let rows = box_area.height.saturating_sub(2) as usize;
    let mut items: Vec<Line> = Vec::new();
    for line in approval.preview.lines().take(rows.saturating_sub(2)) {
        let style = if line == "RISKY" {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        items.push(Line::from(line.to_string()).style(style));
    }
    items.push(Line::from(""));
    items.push(Line::from(format!(
        " y allow once   a allow {} for this session   n deny   Esc deny and abort ",
        approval.tool
    )));
    let title = format!(" approve {} ", approval.tool);
    let modal = Paragraph::new(Text::from(items)).block(Block::bordered().title(title));
    frame.render_widget(modal, box_area);
}

fn centered(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

fn to_line(line: &UiLine) -> Line<'static> {
    let mut style = match line.kind {
        LineKind::User => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        LineKind::Assistant => Style::default(),
        LineKind::Thinking => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        LineKind::ToolCall => Style::default().fg(Color::Yellow),
        LineKind::ToolResult => Style::default().fg(Color::Gray),
        LineKind::Error => Style::default().fg(Color::Red),
        LineKind::Info => Style::default().fg(Color::Magenta),
    };
    if line.dim {
        style = style.fg(Color::DarkGray).add_modifier(Modifier::DIM);
    }
    Line::from(line.text.clone()).style(style)
}
