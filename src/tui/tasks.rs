//! Spawning, aborting and reloading work for the TUI event loop.

use std::path::{Path, PathBuf};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentEvent, Runtime};
use crate::session::Session;

use super::app::App;

pub(super) fn start_task(
    app: &mut App,
    task: &mut Option<JoinHandle<()>>,
    active_path: &mut Option<PathBuf>,
    status: &str,
) {
    if let Some(handle) = task.take() {
        handle.abort();
    }
    *active_path = None;
    app.running = true;
    app.elapsed_ticks = 0;
    app.status = status.to_string();
}

pub(super) fn fail(app: &mut App, text: &str) {
    app.running = false;
    app.status = "ready".to_string();
    app.info(text.to_string());
}

pub(super) fn spawn_turn(
    app: &mut App,
    runtime: &Runtime,
    event_tx: &mpsc::Sender<AgentEvent>,
    done_tx: &mpsc::Sender<Session>,
    text: String,
) -> Option<(JoinHandle<()>, PathBuf)> {
    let mut session = app.session.take()?;
    let path = session.path.clone();
    let runtime = runtime.clone();
    let events = event_tx.clone();
    let errors = event_tx.clone();
    let done = done_tx.clone();
    let (steer_tx, steer_rx) = mpsc::unbounded_channel::<String>();
    app.steer_tx = Some(steer_tx);
    let (approval_tx, approval_rx) = mpsc::channel(4);
    app.approval_tx = Some(approval_tx);

    let handle = tokio::spawn(async move {
        let result = runtime
            .run(&mut session, text, events, steer_rx, approval_rx)
            .await;
        if let Err(err) = result {
            let _ = errors.send(AgentEvent::Error(format!("{err:#}"))).await;
        }
        let _ = done.send(session).await;
    });
    Some((handle, path))
}

pub(super) fn spawn_compact(
    app: &mut App,
    runtime: &Runtime,
    event_tx: &mpsc::Sender<AgentEvent>,
    done_tx: &mpsc::Sender<Session>,
    focus: Option<String>,
) -> Option<(JoinHandle<()>, PathBuf)> {
    let mut session = app.session.take()?;
    let path = session.path.clone();
    let runtime = runtime.clone();
    let events = event_tx.clone();
    let done = done_tx.clone();

    let handle = tokio::spawn(async move {
        let before = runtime.context_tokens(&session);
        match runtime.compact(&mut session, focus).await {
            Ok(true) => {
                let _ = events
                    .send(AgentEvent::Compacted {
                        tokens_before: before,
                    })
                    .await;
            }
            Ok(false) => {
                let _ = events
                    .send(AgentEvent::Error("nothing to compact".to_string()))
                    .await;
            }
            Err(err) => {
                let _ = events.send(AgentEvent::Error(format!("{err:#}"))).await;
            }
        }
        let _ = done.send(session).await;
    });
    Some((handle, path))
}

/// Esc: kill the task, then reload the append-only session file.
pub(super) fn abort_run(
    app: &mut App,
    task: &mut Option<JoinHandle<()>>,
    active_path: &mut Option<PathBuf>,
    event_rx: &mut mpsc::Receiver<AgentEvent>,
) {
    if let Some(handle) = task.take() {
        handle.abort();
    }
    app.approval = None;
    app.approval_tx = None;
    if let Some(path) = active_path.take() {
        match Session::open(&path) {
            Ok(session) => app.set_session(session),
            Err(err) => app.info(format!("cannot reopen session: {err:#}")),
        }
    }
    app.running = false;
    app.status = "aborted".to_string();
    app.steer_tx = None;
    while event_rx.try_recv().is_ok() {}
}

pub(super) fn open_session(app: &mut App, path: &Path, runtime: &Runtime) {
    match Session::open(path) {
        Ok(session) => {
            app.set_session(session);
            app.clear_transcript();
            app.status = "resumed".to_string();
            refresh_tokens(app, runtime);
        }
        Err(err) => app.info(format!("cannot open session: {err:#}")),
    }
}

pub(super) fn refresh_tokens(app: &mut App, runtime: &Runtime) {
    if let Some(session) = &app.session {
        app.context_tokens = runtime.context_tokens(session);
    }
}
