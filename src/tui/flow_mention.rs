//! The `@` file popup: key handling, completion and the sync from the input.

use super::app::App;
use super::keys::{Key, KeyCodeX};
use super::msg::Effect;
use super::picker::{Item, PKey, Picker};

/// Recompute the `@` popup from the word at the cursor.
pub fn sync_mention(app: &mut App) {
    if !app.overlays.is_empty() {
        app.mention = None;
        return;
    }
    let text = app.input.text();
    if app.mention_dismissed.as_deref() == Some(text.as_str()) {
        app.mention = None;
        return;
    }
    app.mention_dismissed = None;
    let Some((start, query)) = super::mention::token_at(&app.input.chars, app.input.cursor) else {
        app.mention = None;
        return;
    };
    if app.files.is_empty() {
        app.mention = None;
        return;
    }
    let matches = super::mention::files_for(&app.files, &query, 8);
    if matches.is_empty() {
        app.mention = None;
        return;
    }
    let selected = app
        .mention
        .as_ref()
        .map(|picker| picker.position().0.saturating_sub(1))
        .unwrap_or(0);
    let items: Vec<Item<String>> = matches
        .into_iter()
        .map(|path| Item::new(path.clone(), path))
        .collect();
    let mut picker = Picker::new(" files ", items);
    picker.select_index(selected);
    app.mention_start = start;
    app.mention = Some(picker);
}

/// Keys owned by the `@` popup. `None` means "let the normal handler run".
pub fn mention_key(app: &mut App, key: Key) -> Option<Vec<Effect>> {
    app.mention.as_ref()?;
    match key.code {
        KeyCodeX::Up => {
            if let Some(picker) = app.mention.as_mut() {
                picker.handle_key(PKey::Up);
            }
            Some(Vec::new())
        }
        KeyCodeX::Down => {
            if let Some(picker) = app.mention.as_mut() {
                picker.handle_key(PKey::Down);
            }
            Some(Vec::new())
        }
        KeyCodeX::Tab | KeyCodeX::Enter => {
            accept_mention(app);
            Some(Vec::new())
        }
        KeyCodeX::Esc => {
            app.mention_dismissed = Some(app.input.text());
            app.mention = None;
            Some(Vec::new())
        }
        _ => None,
    }
}

/// Replace the `@` word with the selected path plus a space.
fn accept_mention(app: &mut App) -> bool {
    let Some(path) = app
        .mention
        .as_ref()
        .and_then(|picker| picker.selected_item().map(|item| item.value.clone()))
    else {
        return false;
    };
    let end = super::mention::token_end(&app.input.chars, app.mention_start);
    app.input
        .replace_range(app.mention_start, end, &format!("@{path} "));
    app.mention = None;
    true
}
