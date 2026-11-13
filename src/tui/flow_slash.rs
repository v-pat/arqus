//! Keys owned by the `/` command popup.

use super::app::App;
use super::keys::{Key, KeyCodeX};
use super::msg::Effect;
use super::slash;

/// Handle one key for the popup. `None` means "let the normal handler run".
pub(super) fn slash_key(app: &mut App, key: Key) -> Option<Vec<Effect>> {
    if !app.slash.open {
        return None;
    }
    match key.code {
        KeyCodeX::Up => {
            app.slash.select_previous();
            Some(Vec::new())
        }
        KeyCodeX::Down => {
            app.slash.select_next();
            Some(Vec::new())
        }
        KeyCodeX::Tab => {
            if let Some(command) = app.slash.selected_command(&app.commands) {
                app.input.set_text(&format!("/{} ", command.name));
            }
            Some(Vec::new())
        }
        KeyCodeX::Enter => {
            let command = app.slash.selected_command(&app.commands).cloned()?;
            app.input.set_text(&format!("/{}", command.name));
            Some(super::flow::submit(app))
        }
        KeyCodeX::Esc => {
            slash::dismiss(&mut app.slash, &app.input.text());
            Some(Vec::new())
        }
        _ => None,
    }
}
