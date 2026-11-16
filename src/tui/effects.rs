//! The effect executor: the only place where a `Msg` turn touches the world.

use std::io::Write;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentEvent, Runtime};
use crate::catalog;
use crate::config;
use crate::resources::Resources;
use crate::safety::Approver;
use crate::session::Session;

use super::footer::Mode;

use super::app::App;
use super::msg::{Effect, Level, Msg};
use super::tasks;

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute(
    effect: Effect,
    app: &mut App,
    runtime: &mut Runtime,
    resources: &Resources,
    event_tx: &mpsc::Sender<AgentEvent>,
    done_tx: &mpsc::Sender<Session>,
    msg_tx: &mpsc::UnboundedSender<Msg>,
    task: &mut Option<JoinHandle<()>>,
    active_path: &mut Option<PathBuf>,
    event_rx: &mut mpsc::Receiver<AgentEvent>,
) {
    match effect {
        Effect::RunPrompt(text) => {
            let expanded = crate::resources::expand_input(resources, &text);
            let expanded = super::mention::expand(&expanded, std::path::Path::new(&app.cwd));
            let spawned = if app.mode == super::footer::Mode::Nanoflow {
                tasks::spawn_nanoflow(app, runtime, event_tx, done_tx, expanded)
            } else {
                tasks::spawn_turn(app, runtime, event_tx, done_tx, expanded)
            };
            match spawned {
                Some((handle, path)) => {
                    *task = Some(handle);
                    *active_path = Some(path);
                }
                None => tasks::fail(app, "no session: use /new"),
            }
        }
        Effect::Compact(focus) => {
            match tasks::spawn_compact(app, runtime, event_tx, done_tx, focus) {
                Some((handle, path)) => {
                    *task = Some(handle);
                    *active_path = Some(path);
                }
                None => tasks::fail(app, "no session to compact"),
            }
        }
        Effect::Abort => tasks::abort_run(app, task, active_path, event_rx),
        Effect::Quit => {}
        Effect::Fork => {
            let result = match app.session.as_ref() {
                Some(session) => crate::tree::fork(session, &app.session_dir),
                None => Err(anyhow::anyhow!("no session")),
            };
            match result {
                Ok(session) => {
                    app.set_session(session);
                    app.replay_session();
                    app.status = "ready".to_string();
                    app.toast("Forked", Level::Info);
                    tasks::refresh_tokens(app, runtime);
                }
                Err(err) => app.info(format!("cannot fork: {err:#}")),
            }
        }
        Effect::NewSession => match Session::create(&app.session_dir, &app.cwd) {
            Ok(session) => {
                let _ = msg_tx.send(Msg::Opened {
                    session,
                    status: "new session".to_string(),
                });
            }
            Err(err) => app.info(format!("cannot create session: {err:#}")),
        },
        Effect::SwitchModel(spec) => {
            let parsed = crate::ai::split_model_spec(&spec);
            let result = match parsed {
                Ok((provider, model)) => config::resolve_for(
                    &provider,
                    &model,
                    &app.env,
                    &app.state,
                    &app.settings,
                    &app.auth,
                )
                .map_err(|err| anyhow::anyhow!(err.to_string())),
                Err(err) => Err(err),
            };
            match result {
                Ok(cfg) => {
                    let applied = match app.session.as_mut() {
                        Some(session) => runtime.apply_model(session, cfg.clone()),
                        None => Err(anyhow::anyhow!("no session")),
                    };
                    match applied {
                        Ok(()) => {
                            app.state.set_model(&cfg.provider, &cfg.model);
                            let path = app.arqus_home.join("state.json");
                            let _ = app.state.save(&path);
                            app.context_window = cfg.context_window;
                            let _ = msg_tx.send(Msg::ModelChanged {
                                provider: cfg.provider,
                                model: cfg.model,
                            });
                        }
                        Err(err) => app.info(format!("cannot switch model: {err:#}")),
                    }
                }
                Err(err) => app.info(format!("cannot switch model: {err:#}")),
            }
        }
        Effect::Budget => {
            let runtime = runtime.clone();
            let events = event_tx.clone();
            tokio::spawn(async move {
                let text = match runtime.budget().await {
                    Ok(text) => text,
                    Err(err) => format!("budget failed: {err:#}"),
                };
                let _ = events.send(AgentEvent::Notice(text)).await;
            });
        }
        Effect::LoadSessions => {
            let dir = app.session_dir.clone();
            let msg_tx = msg_tx.clone();
            tokio::task::spawn_blocking(move || {
                let sessions = super::resume::load(&dir);
                let _ = msg_tx.send(Msg::Sessions(sessions));
            });
        }
        Effect::OpenSession(path) => match Session::open(&path) {
            Ok(session) => {
                let _ = msg_tx.send(Msg::Opened {
                    session,
                    status: "resumed".to_string(),
                });
            }
            Err(err) => app.info(format!("cannot open session: {err:#}")),
        },
        Effect::DeleteSession(path) => {
            let trash = app.session_dir.join("trash");
            let result = std::fs::create_dir_all(&trash).and_then(|()| {
                let name = path
                    .file_name()
                    .ok_or_else(|| std::io::Error::other("session has no file name"))?;
                std::fs::rename(&path, trash.join(name))
            });
            match result {
                Ok(()) => {
                    app.toast("moved to trash", Level::Info);
                    let dir = app.session_dir.clone();
                    let msg_tx = msg_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let sessions = super::resume::load(&dir);
                        let _ = msg_tx.send(Msg::Sessions(sessions));
                    });
                }
                Err(err) => app.toast(format!("cannot delete session: {err:#}"), Level::Error),
            }
        }
        Effect::Export(path) => {
            match super::export::write(&app.blocks, &app.cwd, path.as_deref()) {
                Ok(written) => app.toast(format!("Exported {}", written.display()), Level::Info),
                Err(err) => app.toast(format!("cannot export: {err:#}"), Level::Error),
            }
        }
        Effect::Copy(text) => match super::clipboard::copy_to_clipboard(&text) {
            Ok(()) => app.toast("Copied", Level::Info),
            Err(err) => app.toast(format!("cannot copy: {err:#}"), Level::Error),
        },
        Effect::RefreshTokens => tasks::refresh_tokens(app, runtime),
        Effect::RefreshGit => {
            let cwd = app.cwd.clone();
            let msg_tx = msg_tx.clone();
            tokio::task::spawn_blocking(move || {
                let facts = super::git::read(std::path::Path::new(&cwd));
                let (branch, dirty) = match facts {
                    Some(info) => (Some(info.branch), info.dirty),
                    None => (None, false),
                };
                let _ = msg_tx.send(Msg::Git { branch, dirty });
            });
        }
        Effect::ValidateKey {
            provider,
            key,
            base_url,
        } => {
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let result = super::effects_http::validate_key(&provider, &key, &base_url).await;
                let _ = msg_tx.send(Msg::KeyChecked(result));
            });
        }
        Effect::SaveBaseUrl { provider, base_url } => {
            app.state.base_urls.insert(provider, base_url);
            let path = app.arqus_home.join("state.json");
            if let Err(err) = app.state.save(&path) {
                app.toast(format!("cannot save state: {err:#}"), Level::Error);
            }
        }
        Effect::SaveAuth { provider, key } => {
            app.auth.set_api_key(&provider, &key);
            let path = app.arqus_home.join("auth.json");
            match app.auth.save(&path) {
                Ok(()) => {
                    let name = catalog::find(&provider)
                        .map(|info| info.name.to_string())
                        .unwrap_or_else(|| provider.clone());
                    app.toast(format!("Connected to {name}"), Level::Info);
                }
                Err(err) => app.toast(format!("cannot save the key: {err:#}"), Level::Error),
            }
        }
        Effect::FetchModels { provider } => {
            let base_url = app
                .env
                .base_url
                .clone()
                .or_else(|| app.state.base_urls.get(&provider).cloned());
            let key = catalog::find(&provider)
                .and_then(|info| {
                    catalog::resolve_key(info, &|name| app.env.key(name), &app.auth)
                        .map(|(key, _)| key)
                })
                .or_else(|| app.env.key(catalog::UNIVERSAL_ENV))
                .or_else(|| app.auth.get_key(&provider).map(str::to_string));
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let result = super::effects_http::fetch_models(
                    &provider,
                    base_url.as_deref(),
                    key.as_deref(),
                )
                .await;
                let _ = msg_tx.send(Msg::ModelsLoaded(result));
            });
        }
        Effect::SetModel { provider, id } => {
            let result = config::resolve_for(
                &provider,
                &id,
                &app.env,
                &app.state,
                &app.settings,
                &app.auth,
            );
            match result {
                Ok(cfg) => {
                    let applied = match app.session.as_mut() {
                        Some(session) => runtime.apply_model(session, cfg.clone()),
                        None => Err(anyhow::anyhow!("no session")),
                    };
                    match applied {
                        Ok(()) => {
                            app.state.set_model(&provider, &id);
                            let path = app.arqus_home.join("state.json");
                            if let Err(err) = app.state.save(&path) {
                                app.toast(format!("cannot save state: {err:#}"), Level::Error);
                            }
                            app.context_window = cfg.context_window;
                            let _ = msg_tx.send(Msg::ModelChanged {
                                provider,
                                model: id,
                            });
                        }
                        Err(err) => {
                            app.toast(format!("cannot switch model: {err:#}"), Level::Error)
                        }
                    }
                }
                Err(err) => app.toast(err.to_string(), Level::Error),
            }
        }
        Effect::Disconnect { provider } => {
            if let Some(info) = catalog::find(&provider) {
                if let Some((_, catalog::KeySource::Env(name))) =
                    catalog::resolve_key(info, &|name| app.env.key(name), &app.auth)
                {
                    app.toast(
                        format!("key comes from {name}; unset it to disconnect"),
                        Level::Warn,
                    );
                    return;
                }
            }
            app.auth.remove(&provider);
            if app.current_provider().as_deref() == Some(provider.as_str()) {
                app.provider_state = super::footer::ProviderState::NeedsProvider;
            }
            let path = app.arqus_home.join("auth.json");
            match app.auth.save(&path) {
                Ok(()) => {
                    let name = catalog::find(&provider)
                        .map(|info| info.name.to_string())
                        .unwrap_or_else(|| provider.clone());
                    app.toast(format!("Disconnected from {name}"), Level::Info);
                }
                Err(err) => app.toast(format!("cannot save auth: {err:#}"), Level::Error),
            }
        }
        Effect::OpenUrl(url) => {
            tokio::task::spawn_blocking(move || {
                let program = if cfg!(target_os = "macos") {
                    "open"
                } else {
                    "xdg-open"
                };
                let _ = std::process::Command::new(program).arg(url).spawn();
            });
        }
        Effect::ScanFiles => {
            let cwd = app.cwd.clone();
            let msg_tx = msg_tx.clone();
            tokio::task::spawn_blocking(move || {
                let files = super::files::scan(std::path::Path::new(&cwd));
                let _ = msg_tx.send(Msg::Files(files));
            });
        }
        Effect::RunLocal { command } => {
            let cwd = PathBuf::from(&app.cwd);
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let args = serde_json::json!({ "command": command });
                let output = crate::tools::bash::run(&args, &cwd).await;
                let _ = msg_tx.send(Msg::LocalDone {
                    command,
                    text: output.text,
                    is_error: output.is_error,
                });
            });
        }
        Effect::SaveState => {
            let path = app.arqus_home.join("state.json");
            if let Err(err) = app.state.save(&path) {
                app.toast(format!("cannot save state: {err:#}"), Level::Error);
            }
        }
        Effect::SetMode(mode) => {
            runtime.cfg.plan = mode == Mode::Plan;
            runtime.cfg.approver = match mode {
                Mode::Ask | Mode::Plan => Approver::Interactive,
                Mode::Auto | Mode::Nanoflow => Approver::Auto,
            };
        }
        Effect::SetCompaction(enabled) => {
            runtime.cfg.compaction.enabled = enabled;
        }
        Effect::Bell => {
            let mut out = std::io::stdout();
            let _ = out.write_all(b"\x07");
            let _ = out.flush();
        }
        Effect::SetMouse(on) => super::terminal::set_mouse(on),
    }
}
