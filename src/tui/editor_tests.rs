//! Stage T6 snapshots: editor heights, paste marker, `@` popup, history, queue.

use std::path::PathBuf;

use super::app::App;
use super::footer::{Mode, ProviderState};
use super::keys::parse_key;
use super::msg::Msg;
use super::testing::{assert_snapshot, render_to_string};
use super::update::update;

fn demo() -> App {
    let mut app = App::new(
        PathBuf::from("/tmp"),
        "/Users/me/code/my-project".to_string(),
        "deepseek/deepseek-v4-flash".to_string(),
        128_000,
    );
    app.home = "/Users/me".to_string();
    app.provider_state = ProviderState::Connected {
        provider: "openrouter".to_string(),
        model: "deepseek/deepseek-v4-flash".to_string(),
    };
    app.mode = Mode::Ask;
    app.ticks = 4;
    app.push_user("make the tests pass");
    app
}

fn key(app: &mut App, token: &str) {
    if let Some(key) = parse_key(token) {
        update(app, Msg::Key(key));
    }
}

#[test]
fn editor_one_line_snapshot() {
    let mut app = demo();
    app.input.set_text("fix the failing test");
    let actual = render_to_string(&mut app, 80, 24);
    assert_snapshot("editor-1line-80x24", &actual);
}

#[test]
fn editor_six_lines_snapshot() {
    let mut app = demo();
    app.input.set_text("one\ntwo\nthree\nfour\nfive\nsix");
    let actual = render_to_string(&mut app, 80, 24);
    assert_snapshot("editor-6lines-80x24", &actual);
}

#[test]
fn editor_nine_lines_scrolls_snapshot() {
    let mut app = demo();
    app.input.set_text("1\n2\n3\n4\n5\n6\n7\n8\n9");
    let actual = render_to_string(&mut app, 80, 24);
    assert_snapshot("editor-9lines-80x24", &actual);
}

#[test]
fn big_paste_is_one_dim_line_snapshot() {
    let mut app = demo();
    update(&mut app, Msg::Paste("x".repeat(4210)));
    assert_eq!(app.input.text().len(), 4210);
    let actual = render_to_string(&mut app, 80, 24);
    assert!(
        actual.contains("[pasted 4,210 characters]"),
        "marker line missing:\n{actual}"
    );
    assert_snapshot("editor-paste-marker-80x24", &actual);
}

#[test]
fn at_popup_snapshot() {
    let mut app = demo();
    app.files = vec![
        "README.md".to_string(),
        "src/app.rs".to_string(),
        "src/main.rs".to_string(),
    ];
    update(&mut app, Msg::Paste("look at @sr".to_string()));
    assert!(app.mention.is_some());
    let actual = render_to_string(&mut app, 80, 24);
    assert!(actual.contains("src/main.rs"), "popup missing:\n{actual}");
    assert_snapshot("mention-popup-80x24", &actual);
}

#[test]
fn history_picker_snapshot() {
    let mut app = demo();
    app.input.history.remember("fix the failing test");
    app.input.history.remember("now run cargo test");
    key(&mut app, "ctrl+r");
    assert!(matches!(
        app.overlays.last(),
        Some(super::overlay::Overlay::History(_))
    ));
    let actual = render_to_string(&mut app, 80, 24);
    assert_snapshot("history-picker-80x24", &actual);
}

#[test]
fn queue_lines_snapshot() {
    let mut app = demo();
    app.running = true;
    app.queue = vec![
        "first follow-up".to_string(),
        "second follow-up".to_string(),
    ];
    app.status = "running".to_string();
    let actual = render_to_string(&mut app, 80, 24);
    assert!(actual.contains("queued: first follow-up"), "{actual}");
    assert_snapshot("queue-80x24", &actual);
}

#[test]
fn mode_changes_the_editor_border_color() {
    use super::footer::Mode;
    use super::view::border_color;
    let app = demo();
    let theme = app.theme.clone();
    assert_eq!(border_color(Mode::Ask, &theme), theme.border);
    assert_eq!(border_color(Mode::Auto, &theme), theme.warning);
    assert_eq!(border_color(Mode::Plan, &theme), theme.accent);
}
