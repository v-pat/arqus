//! `/theme` and `/settings`: open the dialogs and apply one changed row.

use super::app::App;
use super::msg::{Effect, Level};
use super::overlay::settings::{Setting, SettingsPicker};
use super::overlay::theme::ThemePicker;
use super::overlay::Overlay;
use super::theme::Theme;
use super::theme_file;

pub(super) fn open_theme(app: &mut App) -> Vec<Effect> {
    let depth = app.theme.depth;
    let (choices, errors) = theme_file::all(&app.arqus_home, depth);
    for error in errors {
        app.toast(format!("theme: {error}"), Level::Warn);
    }
    app.push_overlay(Overlay::Theme(ThemePicker::new(&app.theme, choices)));
    Vec::new()
}

pub(super) fn open_settings(app: &mut App) -> Vec<Effect> {
    let depth = app.theme.depth;
    let (choices, _) = theme_file::all(&app.arqus_home, depth);
    let mut names: Vec<String> = Vec::new();
    for choice in choices {
        if !names.contains(&choice.name) {
            names.push(choice.name);
        }
    }
    app.push_overlay(Overlay::Settings(SettingsPicker::new(app, names)));
    Vec::new()
}

pub(super) fn commit_theme(app: &mut App, theme: Theme, name: String) -> Vec<Effect> {
    app.theme = theme;
    app.state.theme = Some(name);
    vec![Effect::SaveState]
}

pub(super) fn preview_theme(app: &mut App, theme: Theme) {
    app.theme = theme;
}

pub(super) fn restore_theme(app: &mut App, theme: Theme) {
    app.theme = theme;
}

/// Apply one row from `/settings`. Every change is written to `state.json`.
pub(super) fn apply_setting(app: &mut App, setting: Setting, value: &str) -> Vec<Effect> {
    let mut effects = vec![Effect::SaveState];
    match setting {
        Setting::Theme => {
            if let Some(theme) = theme_file::load_named(&app.arqus_home, value, app.theme.depth) {
                app.theme = theme;
                app.state.theme = Some(value.to_string());
            } else {
                app.toast(format!("theme: cannot load {value}"), Level::Warn);
            }
        }
        Setting::ShowThinking => {
            let on = value == "on";
            app.show_thinking = on;
            app.state.show_thinking = on;
        }
        Setting::ExpandTools => {
            app.expanded = value == "on";
            app.state.expand_tool_output = app.expanded;
        }
        Setting::Mouse => {
            let on = value == "on";
            app.state.mouse = on;
            effects.push(Effect::SetMouse(on));
        }
        Setting::Bell => app.state.bell = value == "on",
        Setting::DefaultMode => app.state.default_mode = Some(value.to_string()),
        Setting::Compaction => {
            let on = value == "on";
            app.state.compaction = on;
            effects.push(Effect::SetCompaction(on));
        }
    }
    effects
}
