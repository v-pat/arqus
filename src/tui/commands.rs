//! Slash commands and agent turns started from the prompt box.

use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentEvent, Runtime};
use crate::resources::Resources;
use crate::session::Session;

use super::app::App;
use super::tasks::{fail, refresh_tokens, spawn_compact, spawn_turn, start_task};

const HELP: &str = "\
keys: Left/Right/Home/End move, Ctrl+A/Ctrl+E line ends, Backspace/Delete,
Ctrl+W delete word, Ctrl+U start of line, Ctrl+K end of line.
Enter submits. During a run Enter queues the text; Esc aborts and drops the queue.
Ctrl+J or Alt+Enter inserts a newline. Paste keeps newlines.
Up/Down recall prompts from the first/last line. Tab completes a command.
PageUp/PageDown/Home/End scroll the transcript when the box is empty.
Ctrl+C aborts a run, clears the box, or asks twice to quit. Ctrl+D quits.
commands: /compact [focus] /tree /fork /model [provider:model] /cost /budget
/new /resume /help /quit. /skill:name and /name run a skill or prompt template.";

/// Turn one Enter press into a slash command or an agent turn.
pub(super) fn submit(
    app: &mut App,
    runtime: &mut Runtime,
    resources: &Resources,
    event_tx: &mpsc::Sender<AgentEvent>,
    done_tx: &mpsc::Sender<Session>,
    task: &mut Option<JoinHandle<()>>,
    active_path: &mut Option<PathBuf>,
) {
    let text = app.take_input();
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }
    app.end_follow();
    app.remember_prompt(&text);

    if let Some(rest) = text
        .strip_prefix("/compact")
        .filter(|_| text == "/compact" || text.starts_with("/compact "))
    {
        let focus = rest.trim();
        let focus = if focus.is_empty() {
            None
        } else {
            Some(focus.to_string())
        };
        app.push_user(&text);
        start_task(app, task, active_path, "compacting");
        match spawn_compact(app, runtime, event_tx, done_tx, focus) {
            Some((handle, path)) => {
                *task = Some(handle);
                *active_path = Some(path);
            }
            None => fail(app, "no session to compact"),
        }
    } else if text == "/help" {
        app.info(HELP);
    } else if text == "/tree" {
        app.start_tree();
    } else if text == "/fork" {
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
                refresh_tokens(app, runtime);
            }
            Err(err) => app.info(format!("cannot fork: {err:#}")),
        }
    } else if text == "/model" || text.starts_with("/model ") {
        let spec = text.strip_prefix("/model").unwrap_or("").trim();
        if spec.is_empty() {
            app.info(format!("model: {}", runtime.model_name));
        } else {
            let result = match app.session.as_mut() {
                Some(session) => runtime.switch_model(session, spec),
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
    } else if text == "/cost" {
        let line = match app.session.as_ref() {
            Some(session) => {
                crate::cost::totals_line(&crate::cost::session_totals(&session.entries))
            }
            None => "no session yet".to_string(),
        };
        app.info(line);
    } else if text == "/budget" {
        let runtime = runtime.clone();
        let events = event_tx.clone();
        app.info("checking budget...");
        tokio::spawn(async move {
            let text = match runtime.budget().await {
                Ok(text) => text,
                Err(err) => format!("budget failed: {err:#}"),
            };
            let _ = events.send(AgentEvent::Notice(text)).await;
        });
    } else if text == "/new" {
        match Session::create(&app.session_dir, &app.cwd) {
            Ok(session) => {
                app.set_session(session);
                app.clear_transcript();
                app.context_tokens = 0;
                app.status = "new session".to_string();
            }
            Err(err) => app.info(format!("cannot create session: {err:#}")),
        }
    } else if text == "/quit" || text == "/exit" {
        app.status = "quit".to_string();
    } else if text == "/resume" {
        app.start_resume();
    } else {
        let expanded = crate::resources::expand_input(resources, &text);
        app.push_user(&text);
        start_task(app, task, active_path, "running");
        match spawn_turn(app, runtime, event_tx, done_tx, expanded) {
            Some((handle, path)) => {
                *task = Some(handle);
                *active_path = Some(path);
            }
            None => fail(app, "no session: use /new"),
        }
    }
}
