//! Dialogs drawn over the main screen. A stack of overlays is allowed.

pub mod help;

use ratatui::layout::Rect;
use ratatui::Frame;

use super::keys::{Key, KeyCodeX};
use super::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOutcome {
    None,
    Close,
}

impl Overlay {
    pub fn handle_key(&mut self, key: Key) -> OverlayOutcome {
        match self {
            Overlay::Help => match key.code {
                KeyCodeX::Esc => OverlayOutcome::Close,
                KeyCodeX::Char('?') => OverlayOutcome::Close,
                _ => OverlayOutcome::None,
            },
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        match self {
            Overlay::Help => help::draw(frame, area, theme),
        }
    }
}
