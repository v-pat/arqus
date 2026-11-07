//! Interactive terminal UI: keyboard input, agent events and drawing.

pub mod app;
mod approval;
mod commands;
mod events;
mod history;
mod input;
mod input_lines;
mod keys;
mod layout;
mod picker;
mod scroll;
mod tasks;
mod tree;
pub mod ui;
mod wrap;

use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use ratatui::crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, EventStream, KeyEventKind,
};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{AgentEvent, Runtime};
use crate::resources::Resources;
use crate::session::Session;

use app::App;
use history::History;
use input::InputState;
use keys::handle_key;
use tasks::refresh_tokens;

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
    app.input = InputState::with_history(History::load(
        crate::home::pirs_home().join("history.jsonl"),
    ));
    if let Some(session) = &app.session {
        app.context_tokens = runtime.context_tokens(session);
    }
    if let Some(notice) = crate::safety::git_notice(std::path::Path::new(&app.cwd)) {
        app.info(notice);
    }

    let mut terminal = ratatui::init();
    // Paste arrives as one Event::Paste, not as hundreds of key presses.
    let paste_on = ratatui::crossterm::execute!(stdout(), EnableBracketedPaste).is_ok();
    install_panic_hook();
    let result = event_loop(&mut terminal, &mut app, runtime, startup.resources).await;
    if paste_on {
        let _ = ratatui::crossterm::execute!(stdout(), DisableBracketedPaste);
    }
    ratatui::restore();
    result
}

/// Restore bracketed paste even when the process panics.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ratatui::crossterm::execute!(stdout(), DisableBracketedPaste);
        previous(info);
    }));
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
        app.set_width(size.width as usize);
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            maybe = keys.next() => {
                let Some(Ok(event)) = maybe else { continue };
                match event {
                    Event::Paste(text) => {
                        if app.tree.is_none() && app.resume.is_none() {
                            app.input.insert_text(&text);
                        }
                    }
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        quit = handle_key(
                            key,
                            app,
                            &mut runtime,
                            &resources,
                            &event_tx,
                            &done_tx,
                            &mut task,
                            &mut active_path,
                            &mut event_rx,
                        );
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
                app.steer_tx = None;
                app.approval = None;
                app.approval_tx = None;
                let dropped = app.clear_queue();
                if dropped > 0 {
                    app.info(format!("dropped {dropped} queued message(s)"));
                }
                refresh_tokens(app, &runtime);
            }
            _ = tick.tick() => {
                app.on_tick();
            }
        }
    }

    if let Some(handle) = task.take() {
        handle.abort();
    }
    Ok(())
}
