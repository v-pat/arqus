use super::*;
use crate::tui::app::App;

fn app() -> App {
    App::new(
        std::env::temp_dir(),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    )
}

fn picker(app: &App) -> SettingsPicker {
    let names = ["dark", "light", "mono", "dark (256 colors)"]
        .into_iter()
        .map(str::to_string)
        .collect();
    SettingsPicker::new(app, names)
}

fn key(code: KeyCodeX) -> Key {
    Key::new(code)
}

#[test]
fn every_row_shows_its_current_value() {
    let picker = picker(&app());
    let rows: Vec<_> = picker
        .picker
        .items
        .iter()
        .map(|item| (item.label.as_str(), item.detail.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("theme", "dark"),
            ("show thinking", "off"),
            ("expand tool output", "off"),
            ("mouse capture", "off"),
            ("terminal bell", "on"),
            ("default mode", "ask"),
            ("compaction", "on"),
        ]
    );
}

#[test]
fn theme_cycles_through_the_built_ins() {
    let app = app();
    let mut picker = picker(&app);
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Enter)),
        OverlayOutcome::Setting {
            setting: Setting::Theme,
            value: "light".to_string(),
        }
    );
    assert_eq!(picker.picker.items[0].detail, "light");
}

#[test]
fn booleans_toggle_and_the_detail_follows() {
    let app = app();
    let mut picker = picker(&app);
    picker.handle_key(key(KeyCodeX::Down));
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Enter)),
        OverlayOutcome::Setting {
            setting: Setting::ShowThinking,
            value: "on".to_string(),
        }
    );
    assert_eq!(picker.picker.items[1].detail, "on");
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Enter)),
        OverlayOutcome::Setting {
            setting: Setting::ShowThinking,
            value: "off".to_string(),
        }
    );
}

#[test]
fn default_mode_switches_between_ask_and_auto() {
    let app = app();
    let mut picker = picker(&app);
    for _ in 0..5 {
        picker.handle_key(key(KeyCodeX::Down));
    }
    assert_eq!(
        picker.handle_key(key(KeyCodeX::Enter)),
        OverlayOutcome::Setting {
            setting: Setting::DefaultMode,
            value: "auto".to_string(),
        }
    );
}

#[test]
fn escape_closes_the_dialog() {
    let app = app();
    let mut picker = picker(&app);
    assert_eq!(picker.handle_key(key(KeyCodeX::Esc)), OverlayOutcome::Close);
}
