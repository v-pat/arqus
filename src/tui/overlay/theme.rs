//! The `/theme` picker: moving the selection previews, Enter keeps.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::picker::{to_pkey, Picker, PickerEvent};
use crate::tui::theme::Theme;
use crate::tui::theme_file::NamedTheme;
use crate::tui::widgets::picker_view;

use super::super::keys::{Key, KeyCodeX};
use super::OverlayOutcome;

pub const HINT: &str = "↑↓ preview · enter keep · esc restore";

#[derive(Debug, Clone)]
pub struct ThemePicker {
    pub picker: Picker<NamedTheme>,
    pub previous: Theme,
    previewed: String,
}

impl ThemePicker {
    pub fn new(current: &Theme, choices: Vec<NamedTheme>) -> ThemePicker {
        let items = choices
            .into_iter()
            .map(|choice| {
                let current = choice.name == current.name;
                crate::tui::picker::Item::new(choice.name.clone(), choice).current(current)
            })
            .collect();
        ThemePicker {
            picker: Picker::new("Theme", items),
            previous: current.clone(),
            previewed: current.name.clone(),
        }
    }

    pub fn handle_key(&mut self, key: Key) -> OverlayOutcome {
        if key.code == KeyCodeX::Esc {
            return OverlayOutcome::RestoreTheme(self.previous.clone());
        }
        let Some(pkey) = to_pkey(key) else {
            return OverlayOutcome::None;
        };
        match self.picker.handle_key(pkey) {
            PickerEvent::Selected(choice) => OverlayOutcome::CommitTheme(choice.theme, choice.name),
            PickerEvent::Cancel => OverlayOutcome::RestoreTheme(self.previous.clone()),
            PickerEvent::None => {
                let Some(item) = self.picker.selected_item() else {
                    return OverlayOutcome::None;
                };
                if item.label == self.previewed {
                    return OverlayOutcome::None;
                }
                self.previewed = item.label.clone();
                OverlayOutcome::PreviewTheme(item.value.theme.clone())
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        picker_view::draw(frame, area, &mut self.picker, HINT, theme);
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
