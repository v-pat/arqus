use super::*;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn parse_key_handles_modifiers_and_names() {
    assert_eq!(parse_key("ctrl+p"), Some(Key::ctrl(KeyCodeX::Char('p'))));
    assert_eq!(parse_key("alt+enter"), Some(Key::alt(KeyCodeX::Enter)));
    assert_eq!(parse_key("shift+tab"), Some(Key::shift(KeyCodeX::BackTab)));
    assert_eq!(parse_key("backtab"), Some(Key::new(KeyCodeX::BackTab)));
    assert_eq!(parse_key("f2"), Some(Key::new(KeyCodeX::F(2))));
    assert_eq!(parse_key("?"), Some(Key::new(KeyCodeX::Char('?'))));
    assert_eq!(parse_key("esc"), Some(Key::new(KeyCodeX::Esc)));
    assert_eq!(parse_key("enter"), Some(Key::new(KeyCodeX::Enter)));
    assert_eq!(
        parse_key("ctrl+alt+delete"),
        Some(Key {
            code: KeyCodeX::Delete,
            ctrl: true,
            alt: true,
            shift: false,
        })
    );
    assert_eq!(parse_key("bogus+key"), None);
    assert_eq!(parse_key("nope"), None);
}

#[test]
fn shift_tab_arrives_as_backtab() {
    let event = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(from_crossterm(event), parse_key("shift+tab"));
}

#[test]
fn releases_are_ignored() {
    let mut event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    event.kind = KeyEventKind::Release;
    assert_eq!(from_crossterm(event), None);
}

#[test]
fn default_map_covers_the_common_keys() {
    let map = KeyMap::defaults();
    assert_eq!(
        map.action(parse_key("enter").unwrap()),
        Some(Action::Submit)
    );
    assert_eq!(
        map.action(parse_key("ctrl+j").unwrap()),
        Some(Action::Newline)
    );
    assert_eq!(
        map.action(parse_key("tab").unwrap()),
        Some(Action::TabComplete)
    );
    assert_eq!(
        map.action(parse_key("ctrl+c").unwrap()),
        Some(Action::CtrlC)
    );
    assert_eq!(
        map.action(parse_key("h").unwrap()),
        Some(Action::InputChar('h'))
    );
}
