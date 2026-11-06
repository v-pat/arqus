//! Interactive terminal UI: keyboard input, agent events and drawing.

pub mod app;
mod tasks;
mod tree;
pub mod ui;

use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use ratatui::crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentEvent, Runtime};
use crate::resources::Resources;
use crate::session::Session;

use app::App;
use tasks::{abort_run, fail, open_session, refresh_tokens, spawn_compact, spawn_turn, start_task};

/// Everything `main` hands to the TUI.
pub struct Startup {
    pub runtime: Runtime,
    pub session: Session,
    pub session_dir: PathBuf,
    pub cwd: String,
    pub resources: Resources,
}

pub async fn run(startup: Startup) -> anyhow::Result<()> {
    let model = startup.runtime.model_name.clone();
    let window = startup.runtime.cfg.context_window;
    let runtime = startup.runtime;

    let mut app = App::new(startup.session_dir, startup.cwd, model, window);
    app.set_session(startup.session);
    if let Some(session) = &app.session {
        app.context_tokens = runtime.context_tokens(session);
    }

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, runtime, startup.resources).await;
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    mut runtime: Runtime,
    resources: Resources,
) -> anyhow::Result<()> {
    let mut keys = EventStream::new();
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(256);
    let (done_tx, mut done_rx) = mpsc::channel::<Session>(4);
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut task: Option<JoinHandle<()>> = None;
    let mut active_path: Option<PathBuf> = None;
    let mut quit = false;

    while !quit {
        let size = terminal.size()?;
        app.set_page(size.height.saturating_sub(5) as usize);
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            maybe = keys.next() => {
                let Some(Ok(event)) = maybe else { continue };
                let Event::Key(key) = event else { continue };
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        quit = true;
                    }
                    KeyCode::Esc => {
                        if app.tree.is_some() {
                            app.cancel_tree();
                        } else if app.resume.is_some() {
                            app.cancel_resume();
                        } else if app.running {
                            abort_run(app, &mut task, &mut active_path, &mut event_rx);
                            refresh_tokens(app, &runtime);
                        } else {
                            app.input.clear();
                        }
                    }
                    KeyCode::Enter => {
                        if app.tree.is_some() {
                            if let Some(path) = app.select_tree() {
                                app.info(format!("path: {path}"));
                            }
                        } else if app.resume.is_some() {
                            if let Some(path) = app.pick_resume() {
                                open_session(app, &path, &runtime);
                            }
                        } else if app.running {
                            app.info("busy: wait, or press Esc to abort");
                        } else {
                            submit(app, &mut runtime, &resources, &event_tx, &done_tx, &mut task, &mut active_path);
                            if app.status == "quit" {
                                quit = true;
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::PageUp => app.scroll_up(app.page),
                    KeyCode::PageDown => app.scroll_down(app.page),
                    KeyCode::Up if app.tree.is_some() => app.move_tree(-1),
                    KeyCode::Down if app.tree.is_some() => app.move_tree(1),
                    KeyCode::Up if app.resume.is_some() => app.move_resume(-1),
                    KeyCode::Down if app.resume.is_some() => app.move_resume(1),
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        app.input.push(c);
                    }
                    _ => {}
                }
            }
            Some(event) = event_rx.recv() => {
                app.apply_event(event);
            }
            Some(session) = done_rx.recv() => {
                app.set_session(session);
                app.running = false;
                app.status = "ready".to_string();
                task = None;
                active_path = None;
                refresh_tokens(app, &runtime);
            }
            _ = tick.tick() => {}
        }
    }

    if let Some(handle) = task.take() {
        handle.abort();
    }
    Ok(())
}

/// Turn one Enter press into a slash command or an agent turn.
fn submit(
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
