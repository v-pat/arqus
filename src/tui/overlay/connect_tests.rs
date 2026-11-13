use super::*;
use crate::tui::app::App;
use crate::tui::keys::{Key, KeyCodeX};
use crate::tui::msg::{Effect, Msg};
use crate::tui::overlay::Overlay;
use crate::tui::testing::{assert_snapshot, render_to_string};
use crate::tui::update::update;
use std::path::PathBuf;

const TEST_KEY: &str = "sk-or-v1-TESTKEY123456";

fn app() -> App {
    let mut app = App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        String::new(),
        128_000,
    );
    app.arqus_home = PathBuf::from("/tmp/arqus-test-home");
    app
}

fn connect_app() -> App {
    let mut app = app();
    app.push_overlay(Overlay::Connect(Connect::new(None, app.connect_ctx())));
    app
}

fn key(code: KeyCodeX) -> Msg {
    Msg::Key(Key::new(code))
}

fn enter_key_dialog(app: &mut App) {
    for ch in "open".chars() {
        update(app, key(KeyCodeX::Char(ch)));
    }
    update(app, key(KeyCodeX::Enter));
}

fn connect_step(app: &App) -> &Step {
    match app.overlays.last() {
        Some(Overlay::Connect(connect)) => &connect.step,
        _ => panic!("no connect overlay"),
    }
}

#[test]
fn provider_list_snapshot() {
    let mut app = connect_app();
    let text = render_to_string(&mut app, 80, 24);
    assert_snapshot("connect-providers-80x24", &text);
}

#[test]
fn typing_a_provider_then_enter_opens_the_key_dialog() {
    let mut app = connect_app();
    enter_key_dialog(&mut app);
    match connect_step(&app) {
        Step::EnterKey { provider, .. } => assert_eq!(provider, "openrouter"),
        other => panic!("expected EnterKey, got {other:?}"),
    }
}

#[test]
fn pasted_key_is_cleaned_and_validated() {
    let mut app = connect_app();
    enter_key_dialog(&mut app);
    update(&mut app, Msg::Paste(format!("  {TEST_KEY}  ")));
    let effects = update(&mut app, key(KeyCodeX::Enter));
    assert_eq!(
        effects,
        vec![Effect::ValidateKey {
            provider: "openrouter".to_string(),
            key: TEST_KEY.to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
        }]
    );
    let text = render_to_string(&mut app, 80, 24);
    assert!(!text.contains(TEST_KEY), "snapshot leaked the key");
    assert!(text.contains('•'));
}

#[test]
fn key_ok_saves_auth_then_fetches_models() {
    let mut app = connect_app();
    enter_key_dialog(&mut app);
    update(&mut app, Msg::Paste(TEST_KEY.to_string()));
    update(&mut app, key(KeyCodeX::Enter));
    let effects = update(
        &mut app,
        Msg::KeyChecked(Ok(KeyInfo {
            limit_remaining: Some(12.5),
            limit: Some(20.0),
        })),
    );
    assert_eq!(
        effects,
        vec![
            Effect::SaveAuth {
                provider: "openrouter".to_string(),
                key: TEST_KEY.to_string(),
            },
            Effect::FetchModels {
                provider: "openrouter".to_string(),
            },
        ]
    );
    assert!(matches!(app.overlays.last(), Some(Overlay::Models(_))));
    assert!(!app.toasts.iter().any(|toast| toast.text.contains(TEST_KEY)));
}

#[test]
fn rejected_key_keeps_the_dialog_open() {
    let mut app = connect_app();
    enter_key_dialog(&mut app);
    update(&mut app, Msg::Paste(TEST_KEY.to_string()));
    update(&mut app, key(KeyCodeX::Enter));
    let effects = update(
        &mut app,
        Msg::KeyChecked(Err(KeyError {
            status: Some(401),
            message: "bad key".to_string(),
            network: false,
        })),
    );
    assert!(effects.is_empty());
    match connect_step(&app) {
        Step::EnterKey { status, .. } => match status {
            KeyStatus::Error(message) => assert!(message.contains("Key rejected")),
            other => panic!("expected an error, got {other:?}"),
        },
        other => panic!("expected EnterKey, got {other:?}"),
    }
}

#[test]
fn network_error_offers_save_or_edit() {
    let mut app = connect_app();
    enter_key_dialog(&mut app);
    update(&mut app, Msg::Paste(TEST_KEY.to_string()));
    update(&mut app, key(KeyCodeX::Enter));
    update(
        &mut app,
        Msg::KeyChecked(Err(KeyError {
            status: None,
            message: "timed out".to_string(),
            network: true,
        })),
    );
    assert!(matches!(connect_step(&app), Step::ConfirmSave { .. }));
    let effects = update(&mut app, key(KeyCodeX::Char('y')));
    assert_eq!(effects.len(), 2);
    assert!(matches!(effects[0], Effect::SaveAuth { .. }));
    assert!(matches!(effects[1], Effect::FetchModels { .. }));

    let mut edit = connect_app();
    enter_key_dialog(&mut edit);
    update(&mut edit, Msg::Paste(TEST_KEY.to_string()));
    update(&mut edit, key(KeyCodeX::Enter));
    update(
        &mut edit,
        Msg::KeyChecked(Err(KeyError {
            status: None,
            message: "timed out".to_string(),
            network: true,
        })),
    );
    update(&mut edit, key(KeyCodeX::Char('n')));
    assert!(matches!(connect_step(&edit), Step::EnterKey { .. }));
}

#[test]
fn disconnect_removes_a_saved_key() {
    let mut app = app();
    app.auth.set_api_key("openrouter", "saved");
    app.push_overlay(Overlay::Connect(Connect::new(None, app.connect_ctx())));
    for ch in "open".chars() {
        update(&mut app, key(KeyCodeX::Char(ch)));
    }
    update(&mut app, key(KeyCodeX::Enter));
    assert!(matches!(connect_step(&app), Step::ProviderMenu { .. }));
    update(&mut app, key(KeyCodeX::Down));
    update(&mut app, key(KeyCodeX::Down));
    let effects = update(&mut app, key(KeyCodeX::Enter));
    assert_eq!(
        effects,
        vec![Effect::Disconnect {
            provider: "openrouter".to_string()
        }]
    );
}

#[test]
fn environment_key_cannot_be_removed() {
    let mut app = app();
    app.env
        .keys
        .insert("OPENROUTER_API_KEY".to_string(), "env-key".to_string());
    app.push_overlay(Overlay::Connect(Connect::new(None, app.connect_ctx())));
    for ch in "open".chars() {
        update(&mut app, key(KeyCodeX::Char(ch)));
    }
    update(&mut app, key(KeyCodeX::Enter));
    update(&mut app, key(KeyCodeX::Down));
    update(&mut app, key(KeyCodeX::Down));
    let effects = update(&mut app, key(KeyCodeX::Enter));
    assert!(effects.is_empty());
    match app.overlays.last() {
        Some(Overlay::Connect(connect)) => {
            assert!(connect.notice.as_deref().unwrap_or("").contains("unset"));
        }
        _ => panic!("no connect overlay"),
    }
}

#[test]
fn key_dialog_snapshots() {
    let mut app = connect_app();
    enter_key_dialog(&mut app);
    let empty = render_to_string(&mut app, 80, 24);
    assert_snapshot("connect-key-empty-80x24", &empty);

    update(&mut app, Msg::Paste(TEST_KEY.to_string()));
    let masked = render_to_string(&mut app, 80, 24);
    assert_snapshot("connect-key-masked-80x24", &masked);
    assert!(!masked.contains(TEST_KEY));

    update(&mut app, key(KeyCodeX::Enter));
    let checking = render_to_string(&mut app, 80, 24);
    assert_snapshot("connect-validating-80x24", &checking);

    update(
        &mut app,
        Msg::KeyChecked(Err(KeyError {
            status: Some(401),
            message: "bad key".to_string(),
            network: false,
        })),
    );
    let error = render_to_string(&mut app, 80, 24);
    assert_snapshot("connect-error-80x24", &error);
}
