//! `Ctrl+R`: pick an old prompt and put it back in the editor.

use ratatui::layout::Rect;
use ratatui::Frame;

use super::super::history::History;
use super::super::keys::{Key, KeyCodeX};
use super::super::picker::{to_pkey, Item, Picker, PickerEvent};
use super::super::theme::Theme;
use super::super::widgets::picker_view;
use super::OverlayOutcome;

#[derive(Debug, Clone)]
pub struct HistoryPicker {
    pub picker: Picker<String>,
}

impl HistoryPicker {
    /// Newest first, one row per prompt.
    pub fn new(history: &History) -> HistoryPicker {
        let items: Vec<Item<String>> = history
            .entries()
            .iter()
            .rev()
            .enumerate()
            .map(|(index, text)| {
                let label = first_line(text, 66);
                Item::new(label, text.clone()).detail(format!("#{}", index + 1))
            })
            .collect();
        HistoryPicker {
            picker: Picker::new("history", items),
        }
    }

    pub fn handle_key(&mut self, key: Key) -> OverlayOutcome {
        if key.code == KeyCodeX::Esc {
            return OverlayOutcome::Close;
        }
        let Some(pkey) = to_pkey(key) else {
            return OverlayOutcome::None;
        };
        match self.picker.handle_key(pkey) {
            PickerEvent::Selected(text) => OverlayOutcome::Insert(text),
            PickerEvent::Cancel => OverlayOutcome::Close,
            PickerEvent::None => OverlayOutcome::None,
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        picker_view::draw(
            frame,
            area,
            &mut self.picker,
            "↑↓ select · enter edit · esc cancel",
            theme,
        );
    }
}

fn first_line(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    let mut out: String = line.chars().take(max).collect();
    if line.chars().count() > max {
        out.push('…');
    }
    if out.trim().is_empty() {
        out = "(empty)".to_string();
    }
    out
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
