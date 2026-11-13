//! Dialogs drawn over the main screen. A stack of overlays is allowed.

pub mod connect;
pub mod help;
pub mod history;
pub mod models;
pub mod palette;
pub mod settings;
pub mod theme;

use ratatui::layout::Rect;
use ratatui::Frame;

use super::commands::Command;
use super::keys::{Key, KeyCodeX, KeyMap};
use super::msg::Effect;
use super::picker::{to_pkey, PickerEvent};
use super::theme::Theme;

#[derive(Debug, Clone)]
pub enum Overlay {
    Help,
    Palette(palette::Palette),
    Connect(connect::Connect),
    Models(models::Models),
    History(history::HistoryPicker),
    Theme(theme::ThemePicker),
    Settings(settings::SettingsPicker),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayOutcome {
    None,
    Close,
    /// A palette row was chosen; the caller runs it and closes the dialog.
    Run(Command),
    /// Run these effects and keep the dialog open.
    Effects(Vec<Effect>),
    /// Close the dialog, then run these effects.
    CloseWith(Vec<Effect>),
    /// Close the dialog and put this text in the editor.
    Insert(String),
    /// Replace this dialog with the model picker.
    OpenModels {
        provider: String,
    },
    /// The key is accepted: save it, then open the model picker.
    Connected {
        provider: String,
        key: String,
        base_url: String,
    },
    /// Show this theme while the picker stays open.
    PreviewTheme(Theme),
    /// Keep this theme, remember its name and close.
    CommitTheme(Theme, String),
    /// Put the theme from before the dialog back and close.
    RestoreTheme(Theme),
    /// One settings row changed; the dialog stays open.
    Setting {
        setting: settings::Setting,
        value: String,
    },
}

impl Overlay {
    pub fn handle_key(&mut self, key: Key) -> OverlayOutcome {
        match self {
            Overlay::Help => match key.code {
                KeyCodeX::Esc => OverlayOutcome::Close,
                KeyCodeX::Char('?') => OverlayOutcome::Close,
                _ => OverlayOutcome::None,
            },
            Overlay::Palette(palette) => {
                if key.code == KeyCodeX::Esc {
                    return OverlayOutcome::Close;
                }
                let Some(picker_key) = to_pkey(key) else {
                    return OverlayOutcome::None;
                };
                match palette.picker.handle_key(picker_key) {
                    PickerEvent::Selected(command) => OverlayOutcome::Run(command),
                    PickerEvent::Cancel => OverlayOutcome::Close,
                    PickerEvent::None => OverlayOutcome::None,
                }
            }
            Overlay::Connect(connect) => connect.handle_key(key),
            Overlay::Models(models) => models.handle_key(key),
            Overlay::History(picker) => picker.handle_key(key),
            Overlay::Theme(picker) => picker.handle_key(key),
            Overlay::Settings(picker) => picker.handle_key(key),
        }
    }

    pub fn paste(&mut self, text: &str) {
        match self {
            Overlay::Connect(connect) => connect.paste(text),
            Overlay::Models(models) => models.paste(text),
            _ => {}
        }
    }

    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        keymap: &KeyMap,
        commands: &[Command],
    ) {
        match self {
            Overlay::Help => help::draw(frame, area, theme, keymap, commands),
            Overlay::Palette(palette) => palette.draw(frame, area, theme),
            Overlay::Connect(connect) => connect.draw(frame, area, theme),
            Overlay::Models(models) => models.draw(frame, area, theme),
            Overlay::History(picker) => picker.draw(frame, area, theme),
            Overlay::Theme(picker) => picker.draw(frame, area, theme),
            Overlay::Settings(picker) => picker.draw(frame, area, theme),
        }
    }
}
