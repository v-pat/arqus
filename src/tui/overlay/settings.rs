//! The `/settings` picker: Enter cycles or toggles one setting in place.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::app::App;
use crate::tui::picker::{to_pkey, Item, Picker, PickerEvent};
use crate::tui::theme::Theme;
use crate::tui::widgets::picker_view;

use super::super::keys::{Key, KeyCodeX};
use super::OverlayOutcome;

pub const HINT: &str = "↑↓ select · enter change · esc close · mouse on: shift-drag selects";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Theme,
    ShowThinking,
    ExpandTools,
    Mouse,
    Bell,
    DefaultMode,
    Compaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRow {
    pub setting: Setting,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct SettingsPicker {
    pub picker: Picker<SettingRow>,
    themes: Vec<String>,
}

impl SettingsPicker {
    pub fn new(app: &App, themes: Vec<String>) -> SettingsPicker {
        let theme = app
            .state
            .theme
            .clone()
            .unwrap_or_else(|| app.theme.name.clone());
        let default_mode = app
            .state
            .default_mode
            .clone()
            .unwrap_or_else(|| "ask".to_string());
        let rows = [
            (Setting::Theme, "theme", theme, "Appearance"),
            (
                Setting::ShowThinking,
                "show thinking",
                on_off(app.show_thinking),
                "Appearance",
            ),
            (
                Setting::ExpandTools,
                "expand tool output",
                on_off(app.expanded),
                "Appearance",
            ),
            (
                Setting::Mouse,
                "mouse capture",
                on_off(app.state.mouse),
                "Behaviour",
            ),
            (
                Setting::Bell,
                "terminal bell",
                on_off(app.state.bell),
                "Behaviour",
            ),
            (
                Setting::DefaultMode,
                "default mode",
                default_mode,
                "Behaviour",
            ),
            (
                Setting::Compaction,
                "compaction",
                on_off(app.state.compaction),
                "Behaviour",
            ),
        ];
        let items = rows
            .into_iter()
            .map(|(setting, label, value, group)| {
                Item::new(
                    label,
                    SettingRow {
                        setting,
                        value: value.clone(),
                    },
                )
                .detail(value)
                .group(group)
            })
            .collect();
        SettingsPicker {
            picker: Picker::new("Settings", items),
            themes,
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
            PickerEvent::Selected(row) => {
                let value = self.next_value(&row);
                for item in &mut self.picker.items {
                    if item.value.setting == row.setting {
                        item.value.value = value.clone();
                        item.detail = value.clone();
                    }
                }
                OverlayOutcome::Setting {
                    setting: row.setting,
                    value,
                }
            }
            PickerEvent::Cancel => OverlayOutcome::Close,
            PickerEvent::None => OverlayOutcome::None,
        }
    }

    fn next_value(&self, row: &SettingRow) -> String {
        match row.setting {
            Setting::Theme => {
                if self.themes.is_empty() {
                    return row.value.clone();
                }
                let index = self
                    .themes
                    .iter()
                    .position(|name| name == &row.value)
                    .unwrap_or(0);
                self.themes[(index + 1) % self.themes.len()].clone()
            }
            Setting::DefaultMode => match row.value.as_str() {
                "ask" => "auto".to_string(),
                "auto" => "plan".to_string(),
                "plan" => "nanoflow(experimental)".to_string(),
                _ => "ask".to_string(),
            },
            _ => on_off(row.value != "on"),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        picker_view::draw(frame, area, &mut self.picker, HINT, theme);
    }
}

fn on_off(value: bool) -> String {
    if value {
        "on".to_string()
    } else {
        "off".to_string()
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
