//! Drawing only. All state lives in `App`.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Clear, List, Paragraph};
use ratatui::Frame;

use super::app::{App, LineKind};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .split(frame.area());

    draw_transcript(frame, app, chunks[0]);
    draw_status(frame, app, chunks[1]);
    draw_input(frame, app, chunks[2]);
    if app.resume.is_some() {
        draw_resume(frame, app, frame.area());
    }
    if app.tree.is_some() {
        draw_tree(frame, app, frame.area());
    }
}

fn draw_transcript(frame: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = app.lines.iter().map(to_line).collect();
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
    let text = format!(
        "{} | {}% ({}/{}) | {} | {}",
        app.model, percent, app.context_tokens, app.context_window, file, app.status
    );
    let line = Line::from(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered().title(" prompt ");
    let input = Paragraph::new(app.input.as_str()).block(block);
    frame.render_widget(input, area);

    if app.resume.is_none() && app.tree.is_none() {
        let x = area.x + 1 + app.input.chars().count().min(u16::MAX as usize) as u16;
        let y = area.y + 1;
        frame.set_cursor_position((x.min(area.right().saturating_sub(2)), y));
    }
}

fn draw_resume(frame: &mut Frame, app: &App, area: Rect) {
    let Some(resume) = &app.resume else {
        return;
    };
    let box_area = centered(area, 70, 60);
    frame.render_widget(Clear, box_area);

    let items: Vec<Line> = resume
        .paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let style = if i == resume.selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(format!(" {name}")).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title(" resume: Up/Down, Enter open, Esc cancel "));
    frame.render_widget(list, box_area);
}

fn draw_tree(frame: &mut Frame, app: &App, area: Rect) {
    let Some(state) = &app.tree else {
        return;
    };
    let box_area = centered(area, 80, 70);
    frame.render_widget(Clear, box_area);

    let items: Vec<Line> = state
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
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

    let list = List::new(items)
        .block(Block::bordered().title(" tree: Up/Down, Enter switch, Esc cancel "));
    frame.render_widget(list, box_area);
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

fn to_line(line: &super::app::UiLine) -> Line<'static> {
    let style = match line.kind {
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
    let text = match line.kind {
        LineKind::User => format!("> {}", line.text),
        LineKind::Thinking => format!("  {}", line.text),
        _ => line.text.clone(),
    };
    Line::from(text).style(style)
}
