//! Interactive terminal UI: the Elm loop from `app` to `view` to `Effect`.

mod app;
mod approval;
mod commands;
mod effects;
mod events;
mod fuzzy;
mod history;
mod input;
mod input_lines;
mod keys;
mod layout;
mod msg;
mod overlay;
mod picker;
mod resume;
mod scroll;
mod tasks;
mod theme;
mod tree;
mod update;
mod view;
mod widgets;
mod wrap;

#[cfg(test)]
mod testing;

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
use keys::from_crossterm;
use msg::{Effect, Msg};
use theme::Theme;
use update::update;
use view::view;

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
    app.theme = Theme::from_env();
    app.set_session(startup.session);
    app.input = InputState::with_history(History::load(
        crate::home::arqus_home().join("history.jsonl"),
    ));
    if let Some(session) = &app.session {
        app.context_tokens = runtime.context_tokens(session);
    }
    if let Some(notice) = crate::safety::git_notice(std::path::Path::new(&app.cwd)) {
        app.info(notice);
    }

    let mut terminal = ratatui::init();
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
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Msg>();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut task: Option<JoinHandle<()>> = None;
    let mut active_path: Option<PathBuf> = None;
    let mut quit = false;

    let size = terminal.size()?;
    let _ = update(app, Msg::Resize(size.width, size.height));
    terminal.draw(|frame| view(app, frame))?;
    app.dirty = false;

    while !quit {
        let next = tokio::select! {
            maybe = keys.next() => {
                let Some(Ok(event)) = maybe else { continue };
                match event {
                    Event::Paste(text) => Some(Msg::Paste(text)),
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        from_crossterm(key).map(Msg::Key)
                    }
                    Event::Resize(width, height) => Some(Msg::Resize(width, height)),
                    _ => None,
                }
            }
            Some(event) = event_rx.recv() => Some(Msg::Agent(event)),
            Some(session) = done_rx.recv() => Some(Msg::Done(session)),
            Some(message) = msg_rx.recv() => Some(message),
            _ = tick.tick(), if app.running || app.ctrl_c_tick.is_some() => Some(Msg::Tick),
        };
        let Some(message) = next else { continue };
        quit = process(
            app,
            message,
            &mut runtime,
            &resources,
            &event_tx,
            &done_tx,
            &msg_tx,
            &mut task,
            &mut active_path,
            &mut event_rx,
        )
        .await;
        if app.dirty {
            terminal.draw(|frame| view(app, frame))?;
            app.dirty = false;
        }
    }

    if let Some(handle) = task.take() {
        handle.abort();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process(
    app: &mut App,
    message: Msg,
    runtime: &mut Runtime,
    resources: &Resources,
    event_tx: &mpsc::Sender<AgentEvent>,
    done_tx: &mpsc::Sender<Session>,
    msg_tx: &mpsc::UnboundedSender<Msg>,
    task: &mut Option<JoinHandle<()>>,
    active_path: &mut Option<PathBuf>,
    event_rx: &mut mpsc::Receiver<AgentEvent>,
) -> bool {
    if let Msg::Done(_) = &message {
        if let Some(handle) = task.take() {
            handle.abort();
        }
        *active_path = None;
    }
    let effects = update(app, message);
    for effect in effects {
        if effect == Effect::Quit {
            return true;
        }
        effects::execute(
            effect,
            app,
            runtime,
            resources,
            event_tx,
            done_tx,
            msg_tx,
            task,
            active_path,
            event_rx,
        )
        .await;
    }
    false
}
