//! Tool cards: the header with a state icon, output, diff and write preview.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span as RSpan};

use super::super::block::{truncate_width, ToolBlock, ToolState};
use super::super::styled::display_width;
use super::super::theme::Theme;

const COLLAPSED_LINES: usize = 5;
const EXPANDED_LINES: usize = 200;
const DIFF_LINES: usize = 12;
const WRITE_PREVIEW: usize = 8;

pub fn tool_lines(
    tool: &ToolBlock,
    width: usize,
    theme: &Theme,
    expanded: bool,
) -> Vec<Line<'static>> {
    let (icon, icon_color) = match tool.state {
        ToolState::Running => ("⠋".to_string(), theme.tool),
        ToolState::Ok => (finish_icon("✓", tool), theme.success),
        ToolState::Err => (finish_icon("✗", tool), theme.error),
    };
    let icon_width = display_width(&icon);
    let head = format!("● {}", tool.name);
    let head_width = display_width(&head);
    // `summary_for` includes the tool name; the header already shows it.
    let summary_text = tool
        .summary
        .strip_prefix(tool.name.as_str())
        .map(str::trim_start)
        .unwrap_or(&tool.summary);
    let mut spans = vec![
        RSpan::styled("● ", Style::default().fg(theme.tool)),
        RSpan::styled(
            tool.name.clone(),
            Style::default().fg(theme.tool).add_modifier(Modifier::BOLD),
        ),
    ];
    let mut left_width = head_width;
    let room = width.saturating_sub(head_width + icon_width + 3);
    if room > 2 && !summary_text.is_empty() {
        let summary = truncate_width(summary_text, room);
        left_width += 2 + display_width(&summary);
        spans.push(RSpan::raw("  "));
        spans.push(RSpan::styled(
            summary,
            Style::default().fg(theme.dim).add_modifier(Modifier::DIM),
        ));
    }
    let pad = width.saturating_sub(left_width + icon_width);
    if pad > 0 {
        spans.push(RSpan::raw(" ".repeat(pad)));
    }
    spans.push(RSpan::styled(icon, Style::default().fg(icon_color)));
    let mut out = vec![Line::from(spans)];

    match tool.name.as_str() {
        "edit" if tool.state != ToolState::Running => {
            out.extend(diff_lines(tool, width, theme));
        }
        "write" if tool.state != ToolState::Running => {
            out.extend(write_lines(tool, width, theme));
        }
        _ => out.extend(output_lines(tool, width, theme, expanded)),
    }
    out
}

fn finish_icon(sign: &str, tool: &ToolBlock) -> String {
    match tool.elapsed {
        Some(elapsed) => format!("{sign} {:.1}s", elapsed.as_secs_f64()),
        None => sign.to_string(),
    }
}

fn output_lines(
    tool: &ToolBlock,
    width: usize,
    theme: &Theme,
    expanded: bool,
) -> Vec<Line<'static>> {
    let lines: Vec<&str> = tool.output.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let limit = if expanded {
        EXPANDED_LINES
    } else {
        COLLAPSED_LINES
    };
    let color = if tool.state == ToolState::Err {
        theme.error
    } else {
        theme.fg
    };
    let mut out: Vec<Line<'static>> = lines
        .iter()
        .take(limit)
        .map(|line| {
            gutter_line(
                &truncate_width(line, width.saturating_sub(2)),
                Style::default().fg(color),
                theme,
            )
        })
        .collect();
    let hidden = lines.len().saturating_sub(limit);
    if hidden > 0 {
        let text = if expanded {
            format!("… +{hidden} more lines")
        } else {
            format!("… +{hidden} lines (ctrl+o to expand)")
        };
        out.push(gutter_line(&text, Style::default().fg(theme.dim), theme));
    }
    out
}

fn gutter_line(text: &str, style: Style, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        RSpan::styled("│ ", Style::default().fg(theme.dim)),
        RSpan::styled(text.to_string(), style),
    ])
}

fn diff_lines(tool: &ToolBlock, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let old = tool
        .args
        .get("old_text")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let new = tool
        .args
        .get("new_text")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let mut out = Vec::new();
    out.extend(sign_lines(old, "-", theme.diff_del, width, theme));
    out.extend(sign_lines(new, "+", theme.diff_add, width, theme));
    out
}

fn sign_lines(
    text: &str,
    sign: &str,
    color: Color,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Line<'static>> = lines
        .iter()
        .take(DIFF_LINES)
        .map(|line| {
            let text = truncate_width(&format!("{sign} {line}"), width.saturating_sub(2));
            gutter_line(&text, Style::default().fg(color), theme)
        })
        .collect();
    if lines.len() > DIFF_LINES {
        out.push(gutter_line(
            &format!("… +{} more", lines.len() - DIFF_LINES),
            Style::default().fg(theme.dim),
            theme,
        ));
    }
    out
}

fn write_lines(tool: &ToolBlock, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let content = tool
        .args
        .get("content")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let lines: Vec<&str> = content.lines().collect();
    let style = Style::default().fg(theme.dim).add_modifier(Modifier::DIM);
    let mut out: Vec<Line<'static>> = lines
        .iter()
        .take(WRITE_PREVIEW)
        .map(|line| gutter_line(&truncate_width(line, width.saturating_sub(2)), style, theme))
        .collect();
    if lines.len() > WRITE_PREVIEW {
        out.push(gutter_line(
            &format!("… +{} more lines", lines.len() - WRITE_PREVIEW),
            Style::default().fg(theme.dim),
            theme,
        ));
    }
    out
}

#[cfg(test)]
#[path = "tool_card_tests.rs"]
mod tests;
