//! Stage T6 key flows: `@` mentions, queue recall and `!` local commands.

use super::*;
use crate::tui::keys::{parse_key, Key, KeyCodeX};
use std::path::PathBuf;

fn app() -> App {
    let mut app = App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    );
    app.provider_state = crate::tui::footer::ProviderState::Connected {
        provider: "openrouter".to_string(),
        model: "test-model".to_string(),
    };
    app.auth.set_api_key("openrouter", "test-key");
    app
}

fn key(app: &mut App, token: &str) {
    if let Some(key) = parse_key(token) {
        update(app, Msg::Key(key));
    }
}

#[test]
fn at_popup_completes_a_project_file() {
    let mut app = app();
    app.files = vec!["src/main.rs".to_string(), "README.md".to_string()];
    update(&mut app, Msg::Paste("read @src".to_string()));
    assert!(app.mention.is_some(), "popup should open");
    key(&mut app, "tab");
    assert_eq!(app.input.text(), "read @src/main.rs ");
    assert!(app.mention.is_none());
}

#[test]
fn at_popup_closes_on_escape_and_leaves_the_text() {
    let mut app = app();
    app.files = vec!["src/main.rs".to_string()];
    update(&mut app, Msg::Paste("@sr".to_string()));
    assert!(app.mention.is_some());
    key(&mut app, "esc");
    assert!(app.mention.is_none());
    assert_eq!(app.input.text(), "@sr");
}

#[test]
fn at_without_matching_files_opens_no_popup() {
    let mut app = app();
    app.files = vec!["src/main.rs".to_string()];
    update(&mut app, Msg::Paste("@zzz".to_string()));
    assert!(app.mention.is_none());
}

#[test]
fn alt_up_takes_the_last_queued_prompt_back() {
    let mut app = app();
    app.running = true;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    app.steer_tx = Some(tx);
    app.queue = vec!["one".to_string(), "two".to_string()];
    key(&mut app, "alt+up");
    assert_eq!(app.input.text(), "two");
    assert_eq!(app.queue, vec!["one".to_string()]);
}

#[test]
fn escape_puts_the_whole_queue_back_in_the_editor() {
    let mut app = app();
    app.running = true;
    app.queue = vec!["one".to_string(), "two".to_string()];
    let effects = update(&mut app, Msg::Key(Key::new(KeyCodeX::Esc)));
    assert_eq!(effects, vec![Effect::Abort, Effect::RefreshTokens]);
    assert_eq!(app.input.text(), "one\ntwo");
    assert!(app.queue.is_empty());
}

#[test]
fn bang_commands_run_locally_and_record_one_user_message() {
    let mut app = app();
    let dir = std::env::temp_dir().join(format!("arqus-local-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let session = crate::session::Session::create(&dir, "/tmp").unwrap();
    app.set_session(session);

    update(&mut app, Msg::Paste("!echo hi".to_string()));
    let effects = update(&mut app, Msg::Key(Key::new(KeyCodeX::Enter)));
    assert_eq!(
        effects,
        vec![Effect::RunLocal {
            command: "echo hi".to_string()
        }]
    );
    assert!(!app.running, "a local command never starts the model");
    assert!(matches!(
        app.blocks.last(),
        Some(crate::tui::block::Block::Tool(_))
    ));

    update(
        &mut app,
        Msg::LocalDone {
            command: "echo hi".to_string(),
            text: "hi\n".to_string(),
            is_error: false,
        },
    );
    let session = app.session.as_ref().expect("session");
    match &session.entries.last().expect("entry").kind {
        crate::types::EntryKind::Message {
            message: crate::types::Message::User { text },
        } => assert_eq!(text, "I ran `echo hi` locally. Output:\n\n```text\nhi\n```"),
        other => panic!("expected one user message, got {other:?}"),
    }
    assert_eq!(
        session.entries.len(),
        1,
        "user message only, no assistant entry"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bang_commands_run_even_while_a_run_is_active() {
    let mut app = app();
    app.running = true;
    update(&mut app, Msg::Paste("!pwd".to_string()));
    let effects = update(&mut app, Msg::Key(Key::new(KeyCodeX::Enter)));
    assert_eq!(
        effects,
        vec![Effect::RunLocal {
            command: "pwd".to_string()
        }]
    );
    assert!(app.queue.is_empty());
}
