//! Test helpers: render a frame to text, compare snapshots, replay keys.

use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::app::App;
use super::keys::parse_key;
use super::msg::{Effect, Msg};
use super::update::update;
use super::view::view;

pub fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| view(app, frame))
        .expect("draw test frame");
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

    fn app() -> App {
        App::new(
            PathBuf::from("/tmp"),
            "/tmp".to_string(),
            "test-model".to_string(),
            128_000,
        )
    }

    #[test]
    fn empty_app_snapshot() {
        let mut app = app();
        let actual = render_to_string(&mut app, 80, 24);
        assert_snapshot("empty-80x24", &actual);
    }

    #[test]
    fn help_overlay_snapshot() {
        let mut app = app();
        let effects = replay(&mut app, "?");
        assert!(effects.is_empty());
        let actual = render_to_string(&mut app, 60, 20);
        assert_snapshot("help-60x20", &actual);
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
        assert!(small.contains("hi"));
    }
}
