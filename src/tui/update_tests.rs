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
fn startup_asks_for_git_facts() {
    let mut app = app();
    assert_eq!(update(&mut app, Msg::Started), vec![Effect::RefreshGit]);
}

#[test]
fn git_message_sets_and_clears_the_footer_branch() {
    let mut app = app();
    update(
        &mut app,
        Msg::Git {
            branch: Some("main".to_string()),
            dirty: true,
        },
    );
    let git = app.git.clone().expect("git facts");
    assert_eq!(git.branch, "main");
    assert!(git.dirty);
    update(
        &mut app,
        Msg::Git {
            branch: None,
            dirty: false,
        },
    );
    assert!(app.git.is_none());
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

#[test]
fn slash_menu_filters_completes_and_reopens_after_escape() {
    let mut first = app();
    replay_keys(&mut first, "/");
    assert!(first.slash.open);
    replay_keys(&mut first, "m o");
    assert!(first.slash.open);
    let expected = first
        .slash
        .selected_command(&first.commands)
        .expect("a match")
        .name
        .clone();
    replay_keys(&mut first, "tab");
    assert_eq!(first.input.text(), format!("/{expected} "));
    assert!(!first.slash.open);

    let mut second = app();
    replay_keys(&mut second, "/ m o");
    replay_keys(&mut second, "esc");
    assert!(!second.slash.open);
    assert_eq!(second.input.text(), "/mo");
    replay_keys(&mut second, "d");
    assert!(second.slash.open);
}

#[test]
fn slash_menu_down_then_enter_runs_a_match() {
    let mut app = app();
    replay_keys(&mut app, "/");
    assert!(app.slash.matched.len() > 1);
    replay_keys(&mut app, "down enter");
    assert!(app.input.is_empty());
    assert!(!app.slash.open);
}

#[test]
fn slash_argument_closes_the_popup_and_runs() {
    let mut app = app();
    update(&mut app, Msg::Paste("/mode plan".to_string()));
    assert!(!app.slash.open);
    replay_keys(&mut app, "enter");
    assert_eq!(app.mode, crate::tui::footer::Mode::Plan);
}

#[test]
fn palette_finds_and_runs_the_mode_command() {
    let mut app = app();
    let effects = replay_keys(&mut app, "ctrl+p m o d e enter");
    assert_eq!(
        effects,
        vec![Effect::SetMode(crate::tui::footer::Mode::Auto)]
    );
    assert!(app.overlays.is_empty());
    assert_eq!(app.mode, crate::tui::footer::Mode::Auto);
}

#[test]
fn palette_escape_closes_without_running() {
    let mut app = app();
    replay_keys(&mut app, "ctrl+p m o d e esc");
    assert!(app.overlays.is_empty());
    assert_eq!(app.mode, crate::tui::footer::Mode::Ask);
}

#[test]
fn shift_tab_cycles_the_modes() {
    use crate::tui::footer::Mode;
    let mut app = app();
    assert_eq!(
        replay_keys(&mut app, "shift+tab"),
        vec![Effect::SetMode(Mode::Auto)]
    );
    assert_eq!(app.mode, Mode::Auto);
    assert_eq!(
        replay_keys(&mut app, "shift+tab"),
        vec![Effect::SetMode(Mode::Plan)]
    );
    assert_eq!(app.mode, Mode::Plan);
    assert_eq!(
        replay_keys(&mut app, "shift+tab"),
        vec![Effect::SetMode(Mode::Nanoflow)]
    );
    assert_eq!(app.mode, Mode::Nanoflow);
    assert_eq!(
        replay_keys(&mut app, "shift+tab"),
        vec![Effect::SetMode(Mode::Ask)]
    );
    assert_eq!(app.mode, Mode::Ask);
}

#[test]
fn ctrl_l_opens_the_model_picker() {
    let mut app = app();
    let effects = replay_keys(&mut app, "ctrl+l");
    assert!(matches!(
        app.overlays.last(),
        Some(crate::tui::overlay::Overlay::Models(_))
    ));
    assert_eq!(
        effects,
        vec![Effect::FetchModels {
            provider: "openrouter".to_string()
        }]
    );
}

#[test]
fn theme_preview_follows_the_selection_and_escape_reverts() {
    let mut app = app();
    app.arqus_home =
        std::env::temp_dir().join(format!("arqus_theme_flow_{}", crate::session::new_id()));
    crate::tui::commands::dispatch(&mut app, "/theme");
    update(&mut app, Msg::Key(Key::new(KeyCodeX::Down)));
    assert_eq!(app.theme.name, "light");
    update(&mut app, Msg::Key(Key::new(KeyCodeX::Esc)));
    assert_eq!(app.theme.name, "dark");
    assert!(app.overlays.is_empty());
}

#[test]
fn settings_enter_changes_the_app_and_saves_state() {
    let mut app = app();
    app.arqus_home =
        std::env::temp_dir().join(format!("arqus_settings_flow_{}", crate::session::new_id()));
    crate::tui::commands::dispatch(&mut app, "/settings");
    update(&mut app, Msg::Key(Key::new(KeyCodeX::Down)));
    let effects = update(&mut app, Msg::Key(Key::new(KeyCodeX::Enter)));
    assert!(app.show_thinking);
    assert!(app.state.show_thinking);
    assert_eq!(effects, vec![Effect::SaveState]);
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

#[test]
fn wheel_scrolls_the_transcript() {
    let mut app = app();
    app.set_size(80, 24);
    for index in 0..60 {
        app.push_user(&format!("line {index}"));
    }
    assert!(app.follow);
    update(&mut app, Msg::Wheel(-3));
    assert!(!app.follow);
    let up = app.scroll;
    update(&mut app, Msg::Wheel(3));
    assert!(app.scroll >= up);
}

#[test]
fn a_signal_quits_cleanly() {
    let mut app = app();
    assert_eq!(update(&mut app, Msg::Signal), vec![Effect::Quit]);
}

#[test]
fn a_finished_run_rings_only_after_ten_idle_seconds() {
    let mut app = app();
    app.running = true;
    app.ticks = 200;
    app.last_input_tick = 200;
    let effects = update(
        &mut app,
        Msg::Done(
            crate::session::Session::create(&std::env::temp_dir().join("arqus_bell_test"), "/tmp")
                .expect("session"),
        ),
    );
    assert!(!effects.contains(&Effect::Bell));

    let mut idle = self::app();
    idle.running = true;
    idle.ticks = 200;
    idle.last_input_tick = 10;
    let effects = update(
        &mut idle,
        Msg::Done(
            crate::session::Session::create(&std::env::temp_dir().join("arqus_bell_test"), "/tmp")
                .expect("session"),
        ),
    );
    assert!(effects.contains(&Effect::Bell));
}

#[test]
fn an_accepted_key_says_ok_before_the_model_picker() {
    use crate::tui::overlay::connect::{Connect, KeyInfo, KeyStatus, Step};

    let mut app = app();
    let mut connect = Connect::new(None, app.connect_ctx());
    connect.step = Step::EnterKey {
        provider: "openrouter".to_string(),
        base_url: "http://127.0.0.1:1/v1".to_string(),
        input: String::new(),
        status: KeyStatus::Idle,
    };
    app.push_overlay(Overlay::Connect(connect));

    let effects = update(
        &mut app,
        Msg::KeyChecked(Ok(KeyInfo {
            limit_remaining: Some(9.5),
            limit: Some(10.0),
        })),
    );

    assert!(app.toasts.iter().any(|toast| toast.text == "Key OK"));
    assert!(matches!(app.overlays.last(), Some(Overlay::Models(_))));
    assert!(effects
        .iter()
        .any(|effect| matches!(effect, Effect::FetchModels { .. })));
}
