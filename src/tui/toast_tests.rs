use std::path::PathBuf;

use super::*;
use crate::tui::msg::{Level, Msg};
use crate::tui::update::update;

fn app() -> App {
    App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    )
}

#[test]
fn info_toast_expires_after_three_seconds() {
    let mut app = app();
    update(&mut app, Msg::Toast("saved".to_string(), Level::Info));
    assert_eq!(app.toasts.len(), 1);
    for _ in 0..INFO_TICKS - 1 {
        update(&mut app, Msg::Tick);
    }
    assert_eq!(app.toasts.len(), 1);
    update(&mut app, Msg::Tick);
    assert!(app.toasts.is_empty());
}

#[test]
fn error_toast_expires_after_six_seconds() {
    let mut app = app();
    update(&mut app, Msg::Toast("boom".to_string(), Level::Error));
    for _ in 0..INFO_TICKS {
        update(&mut app, Msg::Tick);
    }
    assert_eq!(app.toasts.len(), 1);
    for _ in 0..INFO_TICKS {
        update(&mut app, Msg::Tick);
    }
    assert!(app.toasts.is_empty());
}

#[test]
fn at_most_three_are_kept_newest_last() {
    let mut app = app();
    for i in 0..5 {
        app.toast(format!("msg {i}"), Level::Info);
    }
    assert_eq!(app.toasts.len(), MAX_VISIBLE);
    assert_eq!(app.toasts.back().map(|t| t.text.as_str()), Some("msg 4"));
    assert_eq!(app.toasts.front().map(|t| t.text.as_str()), Some("msg 2"));
}
