//! The `Ctrl+P` command palette: one `Picker` over the command table.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::commands::Command;
use crate::tui::picker::{Item, Picker};
use crate::tui::theme::Theme;
use crate::tui::widgets::picker_view;

pub const HINT: &str = "↑↓ select · enter run · esc close";

#[derive(Debug, Clone)]
pub struct Palette {
    pub picker: Picker<Command>,
}

impl Palette {
    pub fn new(commands: &[Command]) -> Palette {
        let items = commands
            .iter()
            .map(|command| {
                Item::new(command.label(), command.clone())
                    .detail(command.key_hint.unwrap_or(""))
                    .group(command.group.clone())
            })
            .collect();
        Palette {
            picker: Picker::new("Commands", items),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        picker_view::draw(frame, area, &mut self.picker, HINT, theme);
    }
}
