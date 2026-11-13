use super::*;
use crate::tui::keys::{Key, KeyCodeX};
use crate::tui::theme::ColorDepth;
use crate::tui::theme_file::NamedTheme;

fn choices() -> Vec<NamedTheme> {
    ["dark", "light", "mono"]
        .into_iter()
        .map(|name| NamedTheme {
            name: name.to_string(),
            theme: match name {
                "light" => Theme::light(ColorDepth::True),
                "mono" => Theme::mono(),
                _ => Theme::dark(ColorDepth::True),
            },
        })
        .collect()
}

fn picker() -> ThemePicker {
    ThemePicker::new(&Theme::dark(ColorDepth::True), choices())
}

fn key(code: KeyCodeX) -> Key {
    Key::new(code)
}

#[test]
fn moving_the_selection_previews_and_escape_restores() {
    let mut picker = picker();
    let previous = picker.previous.clone();
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Down)),
        OverlayOutcome::PreviewTheme(Theme::light(ColorDepth::True))
    );
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Down)),
        OverlayOutcome::PreviewTheme(Theme::mono())
    );
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Esc)),
        OverlayOutcome::RestoreTheme(previous)
    );
}

#[test]
fn enter_keeps_the_previewed_theme() {
    let mut picker = picker();
    picker.handle_key(key(KeyCodeX::Down));
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Enter)),
        OverlayOutcome::CommitTheme(Theme::light(ColorDepth::True), "light".to_string())
    );
}

#[test]
fn entering_twice_keeps_the_same_theme() {
    let mut picker = picker();
    picker.handle_key(key(KeyCodeX::Down));
    picker.handle_key(key(KeyCodeX::Enter));
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Enter)),
        OverlayOutcome::CommitTheme(Theme::light(ColorDepth::True), "light".to_string())
    );
}
