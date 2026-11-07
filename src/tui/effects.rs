//! The effect executor: the only place where a `Msg` turn touches the world.

use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentEvent, Runtime};
use crate::resources::Resources;
use crate::session::Session;

use super::app::App;
use super::layout;
use super::msg::{Effect, Msg};
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
            match tasks::spawn_turn(app, runtime, event_tx, done_tx, expanded) {
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
                    let path = session.path.display().to_string();
                    app.set_session(session);
                    app.clear_transcript();
                    app.status = "forked".to_string();
                    app.info(format!("forked to {path}"));
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
            let result = match app.session.as_mut() {
                Some(session) => runtime.switch_model(session, &spec),
                None => Err(anyhow::anyhow!("no session")),
            };
            match result {
                Ok(()) => {
                    app.model = runtime.model_name.clone();
                    app.context_window = runtime.cfg.context_window;
                    app.status = "model changed".to_string();
                    app.info(format!("model: {}", runtime.model_name));
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
            let paths = crate::session::list_sessions(&app.session_dir);
            let sessions = paths
                .into_iter()
                .map(|path| {
                    let time = layout::resume_time(&path);
                    (path, time)
                })
                .collect();
            let _ = msg_tx.send(Msg::Sessions(sessions));
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
        Effect::RefreshTokens => tasks::refresh_tokens(app, runtime),
    }
}
