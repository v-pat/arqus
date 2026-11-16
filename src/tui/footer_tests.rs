use std::path::PathBuf;

use unicode_width::UnicodeWidthStr;

use super::*;
use crate::tui::app::App;
use crate::tui::git::GitInfo;
use crate::tui::testing::{assert_snapshot, render_footer_to_string};

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
    app.git = Some(GitInfo {
        branch: "main".to_string(),
        dirty: true,
    });
    app.context_tokens = 15_360; // 12%
    app.session_cost = Some(0.004);
    app.mode = Mode::Ask;
    app
}

#[test]
fn footer_120_snapshot() {
    let app = demo();
    assert_snapshot("footer-120x1", &render_footer_to_string(&app, 120));
}

#[test]
fn footer_80_snapshot() {
    let app = demo();
    assert_snapshot("footer-80x1", &render_footer_to_string(&app, 80));
}

#[test]
fn footer_60_snapshot() {
    let app = demo();
    assert_snapshot("footer-60x1", &render_footer_to_string(&app, 60));
}

#[test]
fn footer_mode_chip_snapshots() {
    for mode in [Mode::Ask, Mode::Auto, Mode::Plan, Mode::Nanoflow] {
        let mut app = demo();
        app.mode = mode;
        assert_snapshot(
            &format!("footer-mode-{}-80x1", mode.chip()),
            &render_footer_to_string(&app, 80),
        );
    }
}

#[test]
fn ctx_thresholds_are_green_yellow_and_red() {
    assert_eq!(ctx_level(0), CtxLevel::Ok);
    assert_eq!(ctx_level(59), CtxLevel::Ok);
    assert_eq!(ctx_level(60), CtxLevel::Warn);
    assert_eq!(ctx_level(84), CtxLevel::Warn);
    assert_eq!(ctx_level(85), CtxLevel::High);
    assert_eq!(ctx_level(100), CtxLevel::High);
}

#[test]
fn ctx_percent_survives_a_zero_window() {
    assert_eq!(ctx_percent(10, 0), 0);
    assert_eq!(ctx_percent(0, 128_000), 0);
    assert_eq!(ctx_percent(64_000, 128_000), 50);
    assert_eq!(ctx_percent(500_000, 128_000), 100);
}

#[test]
fn home_becomes_tilde_and_long_paths_keep_the_tail() {
    assert_eq!(
        short_path("/Users/me/code/proj", "/Users/me"),
        "~/code/proj"
    );
    assert_eq!(short_path("/Users/me", "/Users/me"), "~");
    assert_eq!(short_path("/tmp/x", "/Users/me"), "/tmp/x");
    assert_eq!(fit_path("~/code/my-project", 10), "…y-project");
    assert_eq!(fit_path("~/code/my-project", 80), "~/code/my-project");
    assert_eq!(
        truncate_end("openrouter/deepseek/model", 12),
        "openrouter/…"
    );
}

#[test]
fn cost_is_short_and_mode_chips_are_stable() {
    assert_eq!(cost_text(0.0), "$0.00");
    assert_eq!(cost_text(0.004), "$0.004");
    assert_eq!(Mode::Ask.chip(), "ask");
    assert_eq!(Mode::Auto.chip(), "auto");
    assert_eq!(Mode::Plan.chip(), "plan");
    assert_eq!(Mode::Nanoflow.chip(), "nanoflow(experimental)");
    assert_eq!(Mode::Ask.next(), Mode::Auto);
    assert_eq!(Mode::Auto.next(), Mode::Plan);
    assert_eq!(Mode::Plan.next(), Mode::Nanoflow);
    assert_eq!(Mode::Nanoflow.next(), Mode::Ask);
    assert_eq!(Mode::parse(" PLAN "), Some(Mode::Plan));
    assert_eq!(Mode::parse("nanoflow"), Some(Mode::Nanoflow));
    assert_eq!(Mode::parse("nanoflow(experimental)"), Some(Mode::Nanoflow));
    assert_eq!(Mode::parse("flow"), Some(Mode::Nanoflow));
    assert_eq!(Mode::parse("nope"), None);
}

#[test]
fn parts_drop_in_the_documented_order() {
    let app = demo();
    let wide = layout(&app, 120);
    assert!(wide.left.iter().any(|p| p.text == "? shortcuts"));
    assert!(wide.left.iter().any(|p| p.text == "main*"));
    assert!(wide.right.iter().any(|p| p.text == "$0.004"));

    let medium = layout(&app, 90);
    assert!(!medium.left.iter().any(|p| p.text == "? shortcuts"));
    assert!(medium.right.iter().any(|p| p.text == "$0.004"));
    assert!(medium.left.iter().any(|p| p.text == "main*"));

    let narrow = layout(&app, 80);
    assert!(!narrow.right.iter().any(|p| p.text == "$0.004"));
    assert!(narrow.left.iter().any(|p| p.text == "main*"));

    let tiny = layout(&app, 60);
    assert!(!tiny.left.iter().any(|p| p.text == "main*"));
    assert!(tiny.left[0].text.width() <= 11);
    assert!(tiny.right[0].text.ends_with('…') || tiny.right[0].text.width() <= 25);
}

#[test]
fn scrolled_state_is_shown_in_the_footer() {
    let mut app = demo();
    app.follow = false;
    let layout = layout(&app, 120);
    assert!(layout
        .left
        .iter()
        .any(|piece| piece.text == "scrolled · End to follow"));
}

#[test]
fn no_provider_is_shown_explicitly() {
    let mut app = app_stub();
    assert!(joined(&layout(&app, 80)).contains("no provider"));
    app.provider_state = ProviderState::Connected {
        provider: "local".to_string(),
        model: "m".to_string(),
    };
    assert!(joined(&layout(&app, 80)).contains("local/m"));
}

fn app_stub() -> App {
    App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "".to_string(),
        128_000,
    )
}

fn joined(layout: &FooterLayout) -> String {
    let left: Vec<&str> = layout.left.iter().map(|p| p.text.as_str()).collect();
    let right: Vec<&str> = layout.right.iter().map(|p| p.text.as_str()).collect();
    format!("{} {}", left.join("  "), right.join("  "))
}
