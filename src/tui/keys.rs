//! Our own key type, parser and default key map.
//!
//! The rest of the TUI never sees a crossterm type.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCodeX {
    Char(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCodeX,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Key {
    pub fn new(code: KeyCodeX) -> Key {
        Key {
            code,
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    pub fn ctrl(code: KeyCodeX) -> Key {
        Key {
            ctrl: true,
            ..Key::new(code)
        }
    }

    pub fn alt(code: KeyCodeX) -> Key {
        Key {
            alt: true,
            ..Key::new(code)
        }
    }

    pub fn shift(code: KeyCodeX) -> Key {
        Key {
            shift: true,
            ..Key::new(code)
        }
    }
}

/// Convert one crossterm event. Key releases are ignored.
pub fn from_crossterm(event: KeyEvent) -> Option<Key> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = event.modifiers.contains(KeyModifiers::ALT);
    let mut shift = event.modifiers.contains(KeyModifiers::SHIFT);
    let code = match event.code {
        KeyCode::Char(c) => KeyCodeX::Char(c),
        KeyCode::Enter => KeyCodeX::Enter,
        KeyCode::Esc => KeyCodeX::Esc,
        KeyCode::Tab => KeyCodeX::Tab,
        KeyCode::BackTab => {
            shift = true;
            KeyCodeX::BackTab
        }
        KeyCode::Backspace => KeyCodeX::Backspace,
        KeyCode::Delete => KeyCodeX::Delete,
        KeyCode::Up => KeyCodeX::Up,
        KeyCode::Down => KeyCodeX::Down,
        KeyCode::Left => KeyCodeX::Left,
        KeyCode::Right => KeyCodeX::Right,
        KeyCode::Home => KeyCodeX::Home,
        KeyCode::End => KeyCodeX::End,
        KeyCode::PageUp => KeyCodeX::PageUp,
        KeyCode::PageDown => KeyCodeX::PageDown,
        KeyCode::F(n) => KeyCodeX::F(n),
        _ => return None,
    };
    Some(Key {
        code,
        ctrl,
        alt,
        shift,
    })
}

/// Parse `ctrl+p`, `alt+enter`, `shift+tab`, `f2` or one character.
pub fn parse_key(text: &str) -> Option<Key> {
    let parts: Vec<&str> = text.split('+').collect();
    let (modifiers, name) = if parts.len() > 1 {
        (&parts[..parts.len() - 1], parts[parts.len() - 1])
    } else {
        (&parts[..0], parts[0])
    };
    if name.is_empty() {
        return None;
    }
    let mut key = Key::new(parse_code(name)?);
    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => key.ctrl = true,
            "alt" | "meta" => key.alt = true,
            "shift" => key.shift = true,
            _ => return None,
        }
    }
    if key.shift && key.code == KeyCodeX::Tab {
        key.code = KeyCodeX::BackTab;
    }
    Some(key)
}

fn parse_code(name: &str) -> Option<KeyCodeX> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "enter" | "return" => return Some(KeyCodeX::Enter),
        "esc" | "escape" => return Some(KeyCodeX::Esc),
        "tab" => return Some(KeyCodeX::Tab),
        "backtab" => return Some(KeyCodeX::BackTab),
        "backspace" => return Some(KeyCodeX::Backspace),
        "delete" | "del" => return Some(KeyCodeX::Delete),
        "up" => return Some(KeyCodeX::Up),
        "down" => return Some(KeyCodeX::Down),
        "left" => return Some(KeyCodeX::Left),
        "right" => return Some(KeyCodeX::Right),
        "home" => return Some(KeyCodeX::Home),
        "end" => return Some(KeyCodeX::End),
        "pageup" => return Some(KeyCodeX::PageUp),
        "pagedown" => return Some(KeyCodeX::PageDown),
        _ => {}
    }
    if let Some(number) = lower.strip_prefix('f') {
        if let Ok(n) = number.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Some(KeyCodeX::F(n));
            }
        }
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(KeyCodeX::Char(c)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    InputChar(char),
    Submit,
    Newline,
    Escape,
    Quit,
    CtrlC,
    CtrlD,
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    DeleteWord,
    KillToStart,
    KillToEnd,
    TabComplete,
    HistoryUp,
    HistoryDown,
    ScrollUp,
    ScrollDown,
    ScrollTop,
    ScrollBottom,
    Help,
}

#[derive(Debug, Clone)]
pub struct KeyMap {
    bindings: Vec<(Key, Action)>,
}

impl KeyMap {
    pub fn defaults() -> KeyMap {
        use Action::*;
        use KeyCodeX as C;
        let mut map = KeyMap {
            bindings: Vec::new(),
        };
        let mut bind = |key: Key, action: Action| map.bindings.push((key, action));
        bind(Key::new(C::Enter), Submit);
        bind(Key::alt(C::Enter), Newline);
        bind(Key::ctrl(C::Char('j')), Newline);
        bind(Key::new(C::Esc), Escape);
        bind(Key::ctrl(C::Char('c')), CtrlC);
        bind(Key::ctrl(C::Char('d')), CtrlD);
        bind(Key::new(C::Backspace), Backspace);
        bind(Key::new(C::Delete), Delete);
        bind(Key::new(C::Tab), TabComplete);
        bind(Key::new(C::Left), MoveLeft);
        bind(Key::new(C::Right), MoveRight);
        bind(Key::alt(C::Left), WordLeft);
        bind(Key::alt(C::Right), WordRight);
        bind(Key::ctrl(C::Char('a')), LineStart);
        bind(Key::ctrl(C::Char('e')), LineEnd);
        bind(Key::ctrl(C::Char('w')), DeleteWord);
        bind(Key::ctrl(C::Char('u')), KillToStart);
        bind(Key::ctrl(C::Char('k')), KillToEnd);
        bind(Key::new(C::Up), HistoryUp);
        bind(Key::new(C::Down), HistoryDown);
        bind(Key::new(C::PageUp), ScrollUp);
        bind(Key::new(C::PageDown), ScrollDown);
        bind(Key::new(C::Home), ScrollTop);
        bind(Key::new(C::End), ScrollBottom);
        map
    }

    pub fn action(&self, key: Key) -> Option<Action> {
        if let Some((_, action)) = self.bindings.iter().find(|(bound, _)| *bound == key) {
            return Some(*action);
        }
        match key.code {
            KeyCodeX::Char(c) if !key.ctrl && !key.alt => Some(Action::InputChar(c)),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
