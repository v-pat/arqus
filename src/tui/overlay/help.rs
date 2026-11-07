//! The `?` shortcuts dialog.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::theme::Theme;
use crate::tui::widgets::dialog::{centered_rect, draw_dialog};

pub const HELP: &str = "\
keys: Left/Right/Home/End move, Ctrl+A/Ctrl+E line ends, Backspace/Delete,
Ctrl+W delete word, Ctrl+U start of line, Ctrl+K end of line.
Enter submits. During a run Enter queues the text; Esc aborts and drops the queue.
Ctrl+J or Alt+Enter inserts a newline. Paste keeps newlines.
Up/Down recall prompts from the first/last line. Tab completes a command.
PageUp/PageDown/Home/End scroll the transcript when the box is empty.
Ctrl+C aborts a run, clears the box, or asks twice to quit. Ctrl+D quits.
? opens this dialog. Esc closes it.
commands: /compact [focus] /tree /fork /model [provider:model] /cost /budget
/new /resume /help /quit. /skill:name and /name run a skill or prompt template.";

const KEYS: &[(&str, &str)] = &[
    ("enter", "send, or queue while running"),
    ("alt+enter / ctrl+j", "newline"),
    ("esc", "close dialog / abort run"),
    ("ctrl+c", "abort / clear / quit twice"),
    ("ctrl+d", "quit on empty input"),
    ("ctrl+a / ctrl+e", "start / end of line"),
    ("ctrl+w / ctrl+u / ctrl+k", "delete word / start / end"),
    ("up / down", "prompt history"),
    ("pageup / pagedown", "scroll transcript"),
    ("home / end", "top / bottom of transcript"),
    ("?", "this dialog"),
];

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme) {
    let width = area.width.saturating_sub(4).clamp(24, 66);
    let height = (KEYS.len() as u16 + 4)
        .min(area.height.saturating_sub(2))
        .max(8);
    let box_area = centered_rect(area, width, height);
    let inner = draw_dialog(frame, box_area, "Shortcuts", "esc close", theme);

    let rows: Vec<Line> = KEYS
        .iter()
        .map(|(key, action)| {
            Line::from(vec![
                Span::styled(
                    format!(" {:<25}", key),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(action.to_string(), Style::default().fg(theme.fg)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(rows)), inner);
}
