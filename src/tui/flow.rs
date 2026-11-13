//! Key handling: the pure part of `update` that turns one key into effects.

use super::app::App;
use super::commands;
use super::keys::{Action, Key, KeyCodeX};
use super::msg::{Effect, Level};
use super::overlay::connect::Connect;
use super::overlay::history::HistoryPicker;
use super::overlay::models::Models;
use super::overlay::palette::Palette;
use super::overlay::{Overlay, OverlayOutcome};

use super::flow_dialogs::{approval_key, resume_key, tree_key};
use super::flow_mention::mention_key;
pub use super::flow_models::{cycle_model, open_models, should_save_base_url};
use super::flow_slash::slash_key;

pub fn on_key(app: &mut App, key: Key) -> Vec<Effect> {
    // The approval modal owns the keyboard: no dialog or key may close it.
    if app.approval.is_some() {
        return approval_key(app, key);
    }
    if let Some(overlay) = app.overlays.last_mut() {
        let outcome = overlay.handle_key(key);
        match outcome {
            OverlayOutcome::Close => {
                app.pop_overlay();
            }
            OverlayOutcome::Run(command) => {
                app.pop_overlay();
                return commands::run(app, &command, "");
            }
            OverlayOutcome::Effects(effects) => return effects,
            OverlayOutcome::Insert(text) => {
                app.pop_overlay();
                app.input.set_text(&text);
                return Vec::new();
            }
            OverlayOutcome::CloseWith(effects) => {
                app.pop_overlay();
                return effects;
            }
            OverlayOutcome::OpenModels { provider } => {
                app.pop_overlay();
                app.push_overlay(Overlay::Models(Models::loading(
                    &provider, &app.state, &app.model,
                )));
                return vec![Effect::FetchModels { provider }];
            }
            OverlayOutcome::Connected {
                provider,
                key,
                base_url,
            } => {
                app.pop_overlay();
                app.push_overlay(Overlay::Models(Models::loading(
                    &provider, &app.state, &app.model,
                )));
                let mut effects = Vec::new();
                if should_save_base_url(&provider, &base_url, app.env.base_url.as_deref()) {
                    effects.push(Effect::SaveBaseUrl {
                        provider: provider.clone(),
                        base_url,
                    });
                }
                effects.push(Effect::SaveAuth {
                    provider: provider.clone(),
                    key,
                });
                effects.push(Effect::FetchModels { provider });
                return effects;
            }
            OverlayOutcome::PreviewTheme(theme) => {
                super::flow_settings::preview_theme(app, theme);
            }
            OverlayOutcome::CommitTheme(theme, name) => {
                app.pop_overlay();
                return super::flow_settings::commit_theme(app, theme, name);
            }
            OverlayOutcome::RestoreTheme(theme) => {
                app.pop_overlay();
                super::flow_settings::restore_theme(app, theme);
            }
            OverlayOutcome::Setting { setting, value } => {
                return super::flow_settings::apply_setting(app, setting, &value);
            }
            OverlayOutcome::None => {}
        }
        return Vec::new();
    }
    if app.resume.is_some() {
        return resume_key(app, key);
    }
    if app.tree.is_some() {
        return tree_key(app, key);
    }
    if key.code == KeyCodeX::Char('?') && !key.ctrl && !key.alt {
        if app.input.is_empty() {
            app.push_overlay(Overlay::Help);
        } else {
            app.input.insert_char('?');
        }
        return Vec::new();
    }
    if let Some(effects) = mention_key(app, key) {
        return effects;
    }
    if let Some(effects) = slash_key(app, key) {
        return effects;
    }
    let Some(action) = app.keymap.action(key) else {
        return Vec::new();
    };
    match action {
        Action::InputChar(c) => app.input.insert_char(c),
        Action::Submit => return submit(app),
        Action::Newline => {
            app.input.insert_newline();
            if key.ctrl && !app.mac_note_shown {
                app.mac_note_shown = true;
                app.toast(
                    "Mac: set Option as Meta for Alt+Enter; Ctrl+J always works",
                    Level::Info,
                );
            }
        }
        Action::Escape => return escape(app),
        Action::Quit => return vec![Effect::Quit],
        Action::CtrlC => return ctrl_c(app),
        Action::CtrlD => {
            if app.input.is_empty() {
                return vec![Effect::Quit];
            }
            app.input.delete();
        }
        Action::Backspace => app.input.backspace(),
        Action::Delete => app.input.delete(),
        Action::MoveLeft => app.input.move_left(),
        Action::MoveRight => app.input.move_right(),
        Action::WordLeft => app.input.word_left(),
        Action::WordRight => app.input.word_right(),
        Action::LineStart => app.input.home(),
        Action::LineEnd => app.input.end(),
        Action::DeleteWord => app.input.delete_word(),
        Action::KillToStart => app.input.kill_to_start(),
        Action::KillToEnd => app.input.kill_to_end(),
        Action::Yank => app.input.yank(),
        Action::TabComplete => {}
        Action::HistoryUp => {
            if app.input.cursor_line() == 0 {
                app.input.history_up();
            }
        }
        Action::HistoryDown => {
            if app.input.cursor_line() + 1 == app.input.line_count() {
                app.input.history_down();
            }
        }
        Action::HistorySearch => {
            let picker = HistoryPicker::new(&app.input.history);
            app.push_overlay(Overlay::History(picker));
        }
        Action::QueueBack => {
            if !app.recall_queued() {
                app.status = "nothing queued".to_string();
            }
        }
        Action::ScrollUp => app.scroll_up(app.page),
        Action::ScrollDown => app.scroll_down(app.page),
        Action::ScrollTop => {
            if app.input.is_empty() {
                app.scroll_top();
            } else {
                app.input.home();
            }
        }
        Action::ScrollBottom => {
            if app.input.is_empty() {
                app.end_follow();
            } else {
                app.input.end();
            }
        }
        Action::Help => app.push_overlay(Overlay::Help),
        Action::Palette => {
            let palette = Palette::new(&app.commands);
            app.push_overlay(Overlay::Palette(palette));
        }
        Action::Models => return open_models(app),
        Action::CycleModel => return cycle_model(app),
        Action::ToggleThinking => {
            app.toggle_thinking();
            app.state.show_thinking = app.show_thinking;
            return vec![Effect::SaveState];
        }
        Action::ToggleToolOutput => app.toggle_expanded(),
        Action::CycleMode => return commands::cycle_mode(app),
    }
    Vec::new()
}

/// Recompute the `@` popup from the word at the cursor.
pub use super::flow_mention::sync_mention;

fn ctrl_c(app: &mut App) -> Vec<Effect> {
    if app.running {
        let dropped = app.clear_queue();
        if dropped > 0 {
            app.info(format!("aborted; dropped {dropped} queued message(s)"));
        }
        vec![Effect::Abort, Effect::RefreshTokens]
    } else if !app.input.is_empty() {
        app.input.clear();
        Vec::new()
    } else if app.ctrl_c_idle() {
        vec![Effect::Quit]
    } else {
        Vec::new()
    }
}

fn escape(app: &mut App) -> Vec<Effect> {
    if app.running {
        if let Some(text) = app.take_queue_text() {
            app.input.set_text(&text);
            app.info("aborted; queued text is back in the editor");
        } else {
            app.info("aborted");
        }
        vec![Effect::Abort, Effect::RefreshTokens]
    } else if !app.input.is_empty() {
        app.input.clear();
        Vec::new()
    } else if app.esc_idle() {
        if app.start_tree() {
            app.status = "tree".to_string();
        }
        Vec::new()
    } else {
        Vec::new()
    }
}

pub(super) fn submit(app: &mut App) -> Vec<Effect> {
    let text = app.take_input();
    let text = text.trim().to_string();
    if text.is_empty() {
        return Vec::new();
    }
    app.end_follow();
    app.remember_prompt(&text);
    if let Some(command) = super::shell::local_command(&text) {
        app.start_local_tool(&command);
        app.status = "running local command".to_string();
        return vec![Effect::RunLocal { command }];
    }
    if app.running {
        if app.queue_prompt(&text) {
            app.status = format!("queued ({} waiting)", app.queue.len());
        } else {
            app.info("cannot queue: no active run");
        }
        return Vec::new();
    }
    if text.starts_with('/') {
        return commands::dispatch(app, &text);
    }
    if app.current_provider().is_none() {
        app.toast("no provider connected", Level::Warn);
        app.push_overlay(Overlay::Connect(Connect::new(None, app.connect_ctx())));
        return Vec::new();
    }
    if app.model.trim().is_empty() {
        app.toast("pick a model first", Level::Warn);
        return open_models(app);
    }
    app.push_user(&text);
    app.running = true;
    app.elapsed_ticks = 0;
    app.status = "running".to_string();
    vec![Effect::RunPrompt(text)]
}
