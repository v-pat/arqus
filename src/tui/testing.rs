//! Test helpers: render a frame to text, compare snapshots, replay keys.

use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::app::App;
use super::footer;
use super::keys::parse_key;
use super::msg::{Effect, Msg};
use super::update::update;
use super::view::view;

pub fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
    app.set_size(width, height);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| view(app, frame))
        .expect("draw test frame");
    buffer_text(&terminal)
}

/// One footer row at `width`, without the rest of the screen.
pub fn render_footer_to_string(app: &App, width: u16) -> String {
    let backend = TestBackend::new(width, 1);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            footer::draw(frame, app, area, &app.theme);
        })
        .expect("draw footer");
    buffer_text(&terminal)
}

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let area = *buffer.area();
    let mut out = String::new();
    for y in 0..area.height {
        let mut line = String::new();
        for x in 0..area.width {
            if let Some(cell) = buffer.cell((x, y)) {
                line.push_str(cell.symbol());
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.trim_end_matches('\n').to_string()
}

pub fn assert_snapshot(name: &str, actual: &str) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let path = dir.join(format!("{name}.txt"));
    let update = std::env::var("UPDATE_SNAPSHOTS")
        .map(|value| value == "1")
        .unwrap_or(false);
    if update || !path.exists() {
        std::fs::create_dir_all(&dir).expect("create snapshot folder");
        std::fs::write(&path, format!("{actual}\n")).expect("write snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path).expect("read snapshot");
    assert_eq!(
        expected.trim_end(),
        actual.trim_end(),
        "snapshot {name} changed; run with UPDATE_SNAPSHOTS=1 to accept\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
    );
}

pub fn replay(app: &mut App, script: &str) -> Vec<Effect> {
    let mut effects = Vec::new();
    for token in script.split_whitespace() {
        let msg = if let Some(text) = token.strip_prefix("paste:") {
            Msg::Paste(text.to_string())
        } else if let Some(key) = parse_key(token) {
            Msg::Key(key)
        } else {
            continue;
        };
        effects.extend(update(app, msg));
    }
    effects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::block::Block;
    use crate::tui::footer::{Mode, ProviderState};
    use crate::tui::git::GitInfo;

    fn app() -> App {
        let mut app = App::new(
            PathBuf::from("/tmp"),
            "/tmp".to_string(),
            "test-model".to_string(),
            128_000,
        );
        app.provider_state = ProviderState::Connected {
            provider: "openrouter".to_string(),
            model: "test-model".to_string(),
        };
        app
    }

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
        app.mode = Mode::Ask;
        app
    }

    #[test]
    fn empty_app_snapshot() {
        let mut app = app();
        app.provider_state = ProviderState::NeedsProvider;
        let actual = render_to_string(&mut app, 80, 24);
        assert_snapshot("empty-80x24", &actual);
    }

    #[test]
    fn welcome_connected_snapshot() {
        let mut app = demo();
        app.session_cost = Some(0.004);
        let actual = render_to_string(&mut app, 80, 24);
        assert_snapshot("welcome-connected-80x24", &actual);
    }

    #[test]
    fn welcome_without_a_provider_snapshot() {
        let mut app = demo();
        app.provider_state = ProviderState::NeedsProvider;
        let actual = render_to_string(&mut app, 80, 24);
        assert_snapshot("welcome-no-provider-80x24", &actual);
    }

    #[test]
    fn chat_view_snapshot() {
        let mut app = demo();
        app.session_cost = Some(0.004);
        app.context_tokens = 15_360;
        app.set_size(80, 24);
        app.push_user("fix the failing test in util.py");
        app.push_block(Block::Assistant {
            text: "I'll run the tests first to see the failure.".to_string(),
            streaming: false,
        });
        let actual = render_to_string(&mut app, 80, 24);
        assert_snapshot("chat-80x24", &actual);
    }

    #[test]
    fn too_small_snapshot() {
        let mut app = demo();
        let actual = render_to_string(&mut app, 40, 10);
        assert_snapshot("too-small-40x10", &actual);
        assert!(actual.contains("terminal too small (need 60x15)"));
    }

    #[test]
    fn help_overlay_snapshot() {
        let mut app = app();
        app.provider_state = ProviderState::NeedsProvider;
        let effects = replay(&mut app, "?");
        assert!(effects.is_empty());
        let actual = render_to_string(&mut app, 60, 20);
        assert_snapshot("help-60x20", &actual);
    }

    #[test]
    fn slash_menu_snapshot() {
        let mut app = demo();
        app.set_size(80, 24);
        app.push_user("hi");
        replay(&mut app, "/ m o");
        assert!(app.slash.open);
        let actual = render_to_string(&mut app, 80, 24);
        assert_snapshot("slash-menu-80x24", &actual);
    }

    #[test]
    fn palette_snapshots() {
        let mut app = demo();
        replay(&mut app, "ctrl+p");
        assert_eq!(app.overlays.len(), 1);
        let wide = render_to_string(&mut app, 80, 24);
        assert_snapshot("palette-80x24", &wide);
        let narrow = render_to_string(&mut app, 60, 20);
        assert_snapshot("palette-60x20", &narrow);
    }

    #[test]
    fn help_overlay_80x24_snapshot() {
        let mut app = demo();
        replay(&mut app, "?");
        let actual = render_to_string(&mut app, 80, 24);
        assert_snapshot("help-80x24", &actual);
    }

    #[test]
    fn theme_picker_snapshots() {
        let mut app = demo();
        app.arqus_home = std::env::temp_dir().join("arqus_theme_snapshot");
        app.theme = crate::tui::theme::Theme::dark(crate::tui::theme::ColorDepth::True);
        crate::tui::commands::dispatch(&mut app, "/theme");
        assert!(matches!(
            app.overlays.last(),
            Some(crate::tui::overlay::Overlay::Theme(_))
        ));
        let wide = render_to_string(&mut app, 80, 24);
        assert_snapshot("theme-picker-80x24", &wide);
        let narrow = render_to_string(&mut app, 60, 20);
        assert_snapshot("theme-picker-60x20", &narrow);
    }

    #[test]
    fn settings_picker_snapshots() {
        let mut app = demo();
        app.arqus_home = std::env::temp_dir().join("arqus_theme_snapshot");
        crate::tui::commands::dispatch(&mut app, "/settings");
        assert!(matches!(
            app.overlays.last(),
            Some(crate::tui::overlay::Overlay::Settings(_))
        ));
        let wide = render_to_string(&mut app, 80, 24);
        assert_snapshot("settings-picker-80x24", &wide);
        let narrow = render_to_string(&mut app, 60, 20);
        assert_snapshot("settings-picker-60x20", &narrow);
    }

    #[test]
    fn typing_hello_then_enter_runs_a_prompt() {
        let mut app = app();
        let effects = replay(&mut app, "h e l l o enter");
        assert_eq!(effects, vec![Effect::RunPrompt("hello".to_string())]);
        assert!(app.running);
    }

    #[test]
    fn question_mark_opens_and_escape_closes_help() {
        let mut app = app();
        replay(&mut app, "?");
        assert_eq!(app.overlays.len(), 1);
        replay(&mut app, "esc");
        assert!(app.overlays.is_empty());
    }

    #[test]
    fn ctrl_c_on_empty_input_starts_the_quit_sequence() {
        let mut app = app();
        assert!(replay(&mut app, "ctrl+c").is_empty());
        assert_eq!(app.status, "press Ctrl+C again to quit");
        assert_eq!(replay(&mut app, "ctrl+c"), vec![Effect::Quit]);
    }

    #[test]
    fn resize_rewraps_and_keeps_the_app_alive() {
        let mut app = app();
        replay(&mut app, "h i");
        let small = render_to_string(&mut app, 40, 12);
        assert!(small.contains("terminal too small"));
        let normal = render_to_string(&mut app, 80, 24);
        assert!(normal.contains("hi"));
    }

    #[test]
    fn one_view_of_five_thousand_blocks_is_fast() {
        let mut app = demo();
        app.set_size(80, 24);
        app.blocks = (0..5_000)
            .map(|index| Block::User(format!("line {index}")))
            .collect();
        app.cache.clear();
        app.refresh_render();
        app.end_follow();
        let started = std::time::Instant::now();
        let _ = render_to_string(&mut app, 80, 24);
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "view took {elapsed:?} for 5,000 blocks"
        );
    }

    #[test]
    fn resize_keeps_the_same_block_at_the_top() {
        let mut app = demo();
        app.set_size(80, 24);
        for index in 0..40 {
            app.push_user(&format!("paragraph number {index} with some words"));
        }
        app.set_size(80, 24);
        app.scroll_up(35);
        let before = app.top_anchor();
        assert!(before.is_some());
        app.set_size(52, 24);
        let after = app.top_anchor();
        assert_eq!(before, after, "the same block stays at the top");
    }
}
