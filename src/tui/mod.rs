//! Interactive terminal UI: the Elm loop from `app` to `view` to `Effect`.

mod actions;
mod app;
mod app_connect;
mod approval;
mod block;
mod clipboard;
mod command_registry;
mod commands;
mod effects;
mod effects_http;
mod event_loop;
mod events;
mod export;
mod facts;
mod files;
mod flow;
mod flow_dialogs;
mod flow_mention;
mod flow_models;
mod flow_settings;
mod flow_slash;
mod footer;
mod fuzzy;
mod git;
mod history;
mod input;
mod input_edit;
mod input_lines;
mod keybinds;
mod keys;
mod layout;
mod markdown;
mod mention;
mod msg;
mod overlay;
mod paste;
mod picker;
mod render;
mod resize;
mod resume;
mod running;
mod scroll;
mod sessions;
mod shell;
mod slash;
mod styled;
mod tasks;
mod terminal;
mod theme;
mod theme_file;
mod toast;
mod tree;
mod update;
mod view;
mod welcome;
mod widgets;
mod wrap;

#[cfg(test)]
mod testing;

#[cfg(test)]
mod editor_tests;

#[cfg(test)]
#[path = "approval_tests.rs"]
mod approval_tests;

#[cfg(test)]
#[path = "mode_tests.rs"]
mod mode_tests;

use std::io::stdout;
use std::path::PathBuf;

use ratatui::crossterm::event::EnableBracketedPaste;

use crate::agent::Runtime;
use crate::auth::AuthStore;
use crate::resources::Resources;
use crate::session::Session;
use crate::settings::{EnvOverrides, Settings};
use crate::state::State;

use app::App;
pub use footer::Mode;
use history::History;
use input::InputState;
use msg::Effect;
use overlay::connect::Connect;
use overlay::models::Models;
use overlay::Overlay;
use theme::ColorDepth;
/// Everything `main` hands to the TUI.
pub struct Startup {
    pub runtime: Runtime,
    pub session: Session,
    pub session_dir: PathBuf,
    pub cwd: String,
    pub resources: Resources,
    /// Provider name from the resolved configuration.
    pub provider: String,
    /// Explicit mode from CLI --mode
    pub initial_mode: Option<footer::Mode>,
    /// True when approvals are off (`--yolo`), so the footer says `auto`.
    pub auto: bool,
    /// Open the Connect dialog on the first frame.
    pub connect_on_start: bool,
    /// Open the model picker on the first frame.
    pub models_on_start: bool,
    /// Open the sessions picker on the first frame.
    pub resume_on_start: bool,
    pub auth: AuthStore,
    pub state: State,
    pub env: EnvOverrides,
    pub settings: Settings,
    pub arqus_home: PathBuf,
}

/// Dialogs that open on the first frame, before any key is pressed.
fn startup_overlays(
    connect_on_start: bool,
    models_on_start: bool,
    provider: &str,
    state: &State,
    model: &str,
    ctx: overlay::connect::ConnectCtx,
) -> Vec<Overlay> {
    if connect_on_start {
        let heading = Some("Welcome to arqus. Connect a provider to start.".to_string());
        vec![Overlay::Connect(Connect::new(heading, ctx))]
    } else if models_on_start {
        vec![Overlay::Models(Models::loading(provider, state, model))]
    } else {
        Vec::new()
    }
}

pub(crate) fn init_app(startup: Startup) -> (App, Runtime, Vec<Effect>) {
    let model = startup.runtime.model_name.clone();
    let window = startup.runtime.cfg.context_window;
    let provider = startup.provider.clone();
    let auto = startup.auto;
    let connect_on_start = startup.connect_on_start;
    let models_on_start = startup.models_on_start;
    let resume_on_start = startup.resume_on_start;
    let mut runtime = startup.runtime;
    let resources = startup.resources;

    let mut app = App::new(startup.session_dir, startup.cwd, model, window);
    app.provider_state = if provider.trim().is_empty() {
        footer::ProviderState::NeedsProvider
    } else {
        footer::ProviderState::Connected {
            provider: provider.clone(),
            model: app.model.clone(),
        }
    };
    app.auth = startup.auth;
    app.state = startup.state;
    app.env = startup.env;
    app.settings = startup.settings;
    app.arqus_home = startup.arqus_home;
    runtime.cfg.compaction.enabled = app.state.compaction;
    app.mode = if let Some(m) = startup.initial_mode {
        m
    } else if runtime.cfg.plan {
        footer::Mode::Plan
    } else if auto || app.state.default_mode.as_deref() == Some("auto") {
        footer::Mode::Auto
    } else if app.state.default_mode.as_deref() == Some("nanoflow")
        || app.state.default_mode.as_deref() == Some("nanoflow(experimental)")
        || app.state.default_mode.as_deref() == Some("flow")
    {
        footer::Mode::Nanoflow
    } else {
        footer::Mode::Ask
    };
    runtime.cfg.plan = app.mode == footer::Mode::Plan;
    runtime.cfg.approver = match app.mode {
        footer::Mode::Ask | footer::Mode::Plan => crate::safety::Approver::Interactive,
        footer::Mode::Auto | footer::Mode::Nanoflow => crate::safety::Approver::Auto,
    };
    app.auto_warned = app.mode == footer::Mode::Auto;
    let depth = ColorDepth::from_env();
    app.theme = theme_file::startup_theme(&app.arqus_home, app.state.theme.as_deref(), depth);
    app.expanded = app.state.expand_tool_output;
    app.commands = commands::with_resources(&resources);
    app.show_thinking = app.state.show_thinking;
    let bindings = keybinds::merge(&app.settings.keybinds);
    app.keymap = bindings.keymap;
    for notice in bindings.notices {
        app.toast(notice, msg::Level::Warn);
    }
    app.set_session(startup.session);
    app.replay_session();
    for overlay in startup_overlays(
        connect_on_start,
        models_on_start,
        &provider,
        &app.state,
        &app.model,
        app.connect_ctx(),
    ) {
        app.push_overlay(overlay);
    }
    app.input = InputState::with_history(History::load(
        crate::home::arqus_home().join("history.jsonl"),
    ));
    if let Some(session) = &app.session {
        app.context_tokens = runtime.context_tokens(session);
    }
    if let Some(notice) = crate::safety::git_notice(std::path::Path::new(&app.cwd)) {
        app.toast(notice, msg::Level::Warn);
    }

    let mut startup_effects = vec![Effect::ScanFiles];
    if models_on_start {
        startup_effects.push(Effect::FetchModels {
            provider: provider.clone(),
        });
    }
    if resume_on_start {
        startup_effects.push(Effect::LoadSessions);
    }
    (app, runtime, startup_effects)
}

pub async fn run(startup: Startup) -> anyhow::Result<()> {
    let resources = startup.resources.clone();
    let (mut app, runtime, startup_effects) = init_app(startup);
    let mut terminal = ratatui::init();
    let paste_on = ratatui::crossterm::execute!(stdout(), EnableBracketedPaste).is_ok();
    let mouse_on = app.state.mouse;
    if mouse_on {
        terminal::set_mouse(true);
    }
    let mut guard = terminal::TerminalGuard::new(paste_on, mouse_on);
    terminal::set_title(&app.cwd);
    terminal::install_panic_hook(paste_on, mouse_on);
    let result = event_loop::event_loop(
        &mut terminal,
        &mut app,
        runtime,
        resources.clone(),
        startup_effects,
    )
    .await;
    // Every exit path, including an error from the loop, restores the screen.
    guard.finish();
    result
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod startup_tests;
