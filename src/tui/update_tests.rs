use super::*;
use crate::tui::keys::parse_key;
use std::path::PathBuf;

fn app() -> App {
    App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    )
}

#[test]
fn typing_then_enter_runs_the_prompt() {
    let mut app = app();
    let effects = replay_keys(&mut app, "h e l l o enter");
    assert_eq!(effects, vec![Effect::RunPrompt("hello".to_string())]);
    assert!(app.running);
    assert!(app.input.is_empty());
}

#[test]
fn question_mark_opens_help_only_on_an_empty_input() {
    let mut app = app();
    replay_keys(&mut app, "h");
    assert!(update(&mut app, Msg::Key(Key::new(KeyCodeX::Char('?')))).is_empty());
    assert!(app.overlays.is_empty());
    assert_eq!(app.input.text(), "h?");

    let mut empty = App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    );
    update(&mut empty, Msg::Key(Key::new(KeyCodeX::Char('?'))));
    assert_eq!(empty.overlays.len(), 1);
    update(&mut empty, Msg::Key(Key::new(KeyCodeX::Esc)));
    assert!(empty.overlays.is_empty());
}

#[test]
fn enter_while_running_queues_the_text() {
    let mut app = app();
    app.running = true;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.steer_tx = Some(tx);
    replay_keys(&mut app, "h i enter");
    assert_eq!(app.queue, vec!["hi".to_string()]);
    assert_eq!(rx.try_recv().unwrap(), "hi");
}

#[test]
fn ctrl_c_first_press_warns_then_quits() {
    let mut app = app();
    assert!(update(&mut app, Msg::Key(Key::ctrl(KeyCodeX::Char('c')))).is_empty());
    assert_eq!(app.status, "press Ctrl+C again to quit");
    assert_eq!(
        update(&mut app, Msg::Key(Key::ctrl(KeyCodeX::Char('c')))),
        vec![Effect::Quit]
    );
}

#[test]
fn ctrl_c_aborts_a_running_turn() {
    let mut app = app();
    app.running = true;
    let effects = update(&mut app, Msg::Key(Key::ctrl(KeyCodeX::Char('c'))));
    assert_eq!(effects, vec![Effect::Abort, Effect::RefreshTokens]);
}

#[test]
fn slash_quit_returns_quit() {
    let mut app = app();
    assert_eq!(replay_keys(&mut app, "/ q u i t enter"), vec![Effect::Quit]);
}

#[test]
fn slash_resume_loads_sessions() {
    let mut app = app();
    assert_eq!(
        replay_keys(&mut app, "/ r e s u m e enter"),
        vec![Effect::LoadSessions]
    );
}

#[test]
fn slash_compact_passes_the_focus() {
    let mut app = app();
    assert_eq!(
        replay_keys(&mut app, "/ c o m p a c t space t e s t enter"),
        vec![Effect::Compact(Some("test".to_string()))]
    );
}

#[test]
fn paste_inserts_text_and_resize_sets_the_size() {
    let mut app = app();
    update(&mut app, Msg::Paste("hello\nworld".to_string()));
    assert_eq!(app.input.text(), "hello\nworld");
    update(&mut app, Msg::Resize(100, 40));
    assert_eq!(app.wrap_width, 100);
    assert_eq!(app.page, 35);
}

fn replay_keys(app: &mut App, script: &str) -> Vec<Effect> {
    let mut effects = Vec::new();
    for token in script.split_whitespace() {
        let msg = if token == "space" {
            Msg::Key(Key::new(KeyCodeX::Char(' ')))
        } else {
            match parse_key(token) {
                Some(key) => Msg::Key(key),
                None => continue,
            }
        };
        effects.extend(update(app, msg));
    }
    effects
}
