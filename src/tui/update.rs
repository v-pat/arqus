//! The pure update function: one `Msg` in, a changed `App` and `Vec<Effect>` out.
//!
//! No I/O, no await, no sleep. Everything that touches the world becomes an
//! `Effect` and runs in `tui::execute`. Key handling lives in `flow`.

use super::app::App;
use super::flow;
use super::msg::{Effect, Msg};
use super::overlay::connect::CheckOutcome;
use super::overlay::models::Models;
use super::overlay::Overlay;
use super::slash;

pub fn update(app: &mut App, msg: Msg) -> Vec<Effect> {
    app.dirty = true;
    match msg {
        Msg::Key(key) => {
            app.mark_activity();
            let effects = flow::on_key(app, key);
            slash::sync(&mut app.slash, &app.input.text(), &app.commands);
            flow::sync_mention(app);
            effects
        }
        Msg::Paste(text) => {
            app.mark_activity();
            if let Some(overlay) = app.overlays.last_mut() {
                overlay.paste(&text);
            } else if app.approval.is_none() && app.tree.is_none() && app.resume.is_none() {
                app.input.insert_paste(&text);
                slash::sync(&mut app.slash, &app.input.text(), &app.commands);
                flow::sync_mention(app);
            }
            Vec::new()
        }
        Msg::Resize(width, height) => {
            // The size is already debounced by the event loop.
            app.cache.clear();
            app.set_size(width, height);
            app.refresh_render();
            if app.follow {
                app.follow_bottom();
            }
            Vec::new()
        }
        Msg::Wheel(delta) => {
            if delta < 0 {
                app.scroll_up(delta.unsigned_abs() as usize);
            } else {
                app.scroll_down(delta as usize);
            }
            Vec::new()
        }
        Msg::Signal => vec![Effect::Quit],
        Msg::Tick => {
            app.on_tick();
            Vec::new()
        }
        Msg::Started => vec![Effect::RefreshGit],
        Msg::Agent(event) => {
            if app.apply_event(event) && app.state.bell {
                vec![Effect::Bell]
            } else {
                Vec::new()
            }
        }
        Msg::Done(session) => {
            app.set_session(session);
            app.running = false;
            app.status = "ready".to_string();
            app.steer_tx = None;
            app.approval = None;
            app.approval_tx = None;
            app.retry_notice = None;
            let dropped = app.clear_queue();
            if dropped > 0 {
                app.info(format!("dropped {dropped} queued message(s)"));
            }
            let mut effects = vec![Effect::RefreshTokens, Effect::RefreshGit];
            // A finished run rings only when the window sat idle for 10s.
            let idle = app.ticks.saturating_sub(app.last_input_tick) >= 100;
            if app.state.bell && idle {
                effects.push(Effect::Bell);
            }
            effects
        }
        Msg::Git { branch, dirty } => {
            app.git = branch.map(|branch| super::git::GitInfo { branch, dirty });
            Vec::new()
        }
        Msg::Toast(text, level) => {
            app.toast(text, level);
            Vec::new()
        }
        Msg::Sessions(sessions) => {
            app.show_resume(sessions);
            Vec::new()
        }
        Msg::Opened { session, status } => {
            app.set_session(session);
            app.clear_transcript();
            app.replay_session();
            app.status = status;
            vec![Effect::RefreshTokens]
        }
        Msg::KeyChecked(result) => {
            let outcome = match app.overlays.last_mut() {
                Some(Overlay::Connect(connect)) => connect.on_key_checked(result),
                _ => CheckOutcome::Pending,
            };
            match outcome {
                CheckOutcome::Ok {
                    provider,
                    key,
                    base_url,
                } => {
                    app.toast("Key OK", super::msg::Level::Info);
                    app.pop_overlay();
                    app.push_overlay(Overlay::Models(Models::loading(
                        &provider, &app.state, &app.model,
                    )));
                    let mut effects = Vec::new();
                    if flow::should_save_base_url(&provider, &base_url, app.env.base_url.as_deref())
                    {
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
                    effects
                }
                _ => Vec::new(),
            }
        }
        Msg::ModelsLoaded(result) => {
            if let Some(Overlay::Models(models)) = app.overlays.last_mut() {
                models.on_loaded(result);
            }
            Vec::new()
        }
        Msg::ModelChanged { provider, model } => {
            app.model = model.clone();
            app.provider_state = super::footer::ProviderState::Connected {
                provider: provider.clone(),
                model: model.clone(),
            };
            app.toast(format!("Model: {model}"), super::msg::Level::Info);
            Vec::new()
        }
        Msg::Files(files) => {
            app.files = files;
            Vec::new()
        }
        Msg::LocalDone {
            command,
            text,
            is_error,
        } => {
            app.finish_local_tool(&command, &text, is_error);
            app.status = "ready".to_string();
            Vec::new()
        }
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "update_editor_tests.rs"]
mod editor_tests;
