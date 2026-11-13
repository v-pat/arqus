use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use ratatui::crossterm::event::{Event, EventStream, KeyEventKind, MouseEventKind};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::agent::{AgentEvent, Runtime};
use crate::resources::Resources;
use crate::session::Session;

use super::app::App;
use super::effects;
use super::keys::from_crossterm;
use super::msg::{Effect, Msg};
use super::resize::ResizeDebounce;
use super::update::update;
use super::view::view;

/// Longest gap between two draws while a run streams (30 fps cap).
const FRAME: Duration = Duration::from_millis(33);

pub(super) async fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    mut runtime: Runtime,
    resources: Resources,
    startup_effects: Vec<Effect>,
) -> anyhow::Result<()> {
    let mut keys = EventStream::new();
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(256);
    let (done_tx, mut done_rx) = mpsc::channel::<Session>(4);
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Msg>();
    watch_quit_signals(&msg_tx);
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut task: Option<JoinHandle<()>> = None;
    let mut active_path: Option<PathBuf> = None;
    let mut resize = ResizeDebounce::default();
    let started = Instant::now();
    let mut last_draw: Option<Instant> = None;
    let mut quit = false;

    let size = terminal.size()?;
    let _ = update(app, Msg::Resize(size.width, size.height));
    process(
        app,
        Msg::Started,
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
    for effect in startup_effects {
        effects::execute(
            effect,
            app,
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
    }
    draw(terminal, app, &mut last_draw).await?;

    while !quit {
        let deadline = resize
            .deadline_ms()
            .map(|ms| started + Duration::from_millis(ms));
        let next = tokio::select! {
            maybe = keys.next() => match maybe {
                Some(Ok(Event::Resize(width, height))) => {
                    let now = started.elapsed().as_millis() as u64;
                    resize.push(width, height, now);
                    None
                }
                Some(Ok(event)) => map_event(event),
                // A closed stdin (EOF) or a broken stream quits cleanly.
                Some(Err(_)) | None => Some(Msg::Signal),
            },
            Some(event) = event_rx.recv() => Some(Msg::Agent(event)),
            Some(session) = done_rx.recv() => Some(Msg::Done(session)),
            Some(message) = msg_rx.recv() => Some(message),
            _ = tokio::time::sleep_until(deadline.unwrap_or(started)), if deadline.is_some() => {
                let now = started.elapsed().as_millis() as u64;
                resize.ready(now).map(|(width, height)| Msg::Resize(width, height))
            }
            _ = tick.tick(), if app.running || app.ctrl_c_tick.is_some() || app.esc_tick.is_some() || !app.toasts.is_empty() => Some(Msg::Tick),
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
        draw(terminal, app, &mut last_draw).await?;
    }

    if let Some(handle) = task.take() {
        handle.abort();
    }
    Ok(())
}

/// SIGTERM and SIGHUP send `Msg::Quit` instead of killing the process with the
/// terminal still in raw mode.
fn watch_quit_signals(msg_tx: &mpsc::UnboundedSender<Msg>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let msg_tx = msg_tx.clone();
        tokio::spawn(async move {
            let mut term = match signal(SignalKind::terminate()) {
                Ok(stream) => stream,
                Err(_) => return,
            };
            let mut hup = match signal(SignalKind::hangup()) {
                Ok(stream) => stream,
                Err(_) => return,
            };
            tokio::select! {
                _ = term.recv() => {}
                _ = hup.recv() => {}
            }
            let _ = msg_tx.send(Msg::Signal);
        });
    }
    #[cfg(not(unix))]
    let _ = msg_tx;
}

fn map_event(event: Event) -> Option<Msg> {
    match event {
        Event::Paste(text) => Some(Msg::Paste(text)),
        Event::Key(key) if key.kind == KeyEventKind::Press => from_crossterm(key).map(Msg::Key),
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollUp => Some(Msg::Wheel(-3)),
            MouseEventKind::ScrollDown => Some(Msg::Wheel(3)),
            _ => None,
        },
        _ => None,
    }
}

/// Draw only when something changed, and never faster than 30 fps while a
/// run streams. The loop still waits on events between frames.
async fn draw(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    last_draw: &mut Option<Instant>,
) -> anyhow::Result<()> {
    if !app.dirty {
        return Ok(());
    }
    if app.running {
        if let Some(previous) = *last_draw {
            let elapsed = previous.elapsed();
            if elapsed < FRAME {
                tokio::time::sleep(FRAME - elapsed).await;
            }
        }
    }
    terminal.draw(|frame| view(app, frame))?;
    app.dirty = false;
    *last_draw = Some(Instant::now());
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
