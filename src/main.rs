#![allow(dead_code)] // remove this line once the modules are used by the agent

mod agent;
mod ai;
mod atomic_write;
mod auth;
mod catalog;
mod cli;
mod compaction;
mod config;
mod context;
mod cost;
mod debug;
mod edit_match;
mod flow;
mod home;
mod mcp;
mod models;
mod print;
mod process_group;
mod resources;
mod safety;
mod safety_setup;
mod session;
mod settings;
mod settings_keybinds;
mod state;
mod system_prompt;
mod tools;
mod tree;
mod tui;
mod types;

use std::io::IsTerminal;

use anyhow::Context;
use clap::Parser;

use crate::agent::AgentConfig;
use crate::ai::{Llm, ModelConfig};
use crate::cli::{resolve_limits, Cli};
use crate::safety::Approvals;
use crate::session::{latest_session, new_id, sessions_dir, Session};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    debug::init(cli.debug, &home::arqus_home());
    if let Some(crate::cli::Command::Flow(ref flow_args)) = cli.command {
        if let Err(err) = flow::cli::run(flow_args).await {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(prompt) = cli.prompt.clone() {
        if let Err(err) = print::print_mode(&cli, prompt).await {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
        return;
    }
    // The interactive mode needs a real terminal; scripts use -p.
    if !std::io::stdout().is_terminal() {
        eprintln!("arqus needs a terminal; use -p \"prompt\" for scripts");
        std::process::exit(2);
    }
    if let Err(err) = interactive_mode(&cli).await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

async fn interactive_mode(cli: &Cli) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read current directory")?;
    let cwd_str = cwd.to_string_lossy().to_string();

    let settings = settings::Settings::load(&cwd);
    let arqus_home = home::arqus_home();
    let state = state::State::load(&arqus_home.join("state.json"));
    let auth = load_auth(&arqus_home);
    let env = settings::EnvOverrides::from_env();
    let resolved = config::resolve_model(cli.model.as_deref(), &env, &state, &settings, &auth);
    let (model, connect_on_start, models_on_start) = match resolved {
        Ok(model) => (model, false, false),
        Err(config::ConfigError::NoProvider) => (placeholder_model(""), true, false),
        Err(config::ConfigError::NoModel { provider }) => {
            (placeholder_model(&provider), false, true)
        }
        Err(config::ConfigError::NoKey { provider }) => (placeholder_model(&provider), true, false),
        Err(err) => exit_config(err.to_string()),
    };
    let limits = resolve_limits(cli, &settings);
    let resources = resources::load_with(&cwd, &settings.skill_folders);
    let session = open_session(cli, &cwd_str)?;
    let session_dir = if cli.no_session {
        session
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| sessions_dir(&cwd_str))
    } else {
        sessions_dir(&cwd_str)
    };

    let (approver, plan) =
        match safety_setup::resolve_mode(cli.mode.as_deref(), cli.yolo, &settings, true) {
            Ok(res) => res,
            Err(err) => exit_config(err),
        };
    let auto = approver == safety::Approver::Auto;

    let cfg = AgentConfig {
        cwd: cwd.clone(),
        system_prompt: system_prompt::build_system_prompt(
            &resources,
            &cwd_str,
            &system_prompt::today_utc(),
        ),
        context_window: model.context_window,
        compaction: settings.compaction_settings(compaction::CompactionSettings::default()),
        limits,
        approver,
        allow_risky: cli.allow_risky,
        plan,
        approvals: Approvals::default(),
        guard: safety_setup::guard(&settings, &cwd),
        mcp: Some(mcp::McpManager::for_cwd(&cwd)),
    };
    let model_name = if model.model.is_empty() {
        "(no model)".to_string()
    } else {
        model.model.clone()
    };
    let runtime = agent::Runtime {
        llm: Llm::from_config(model.clone()),
        cfg,
        model_name,
    };
    let provider = model.provider.clone();

    tui::run(tui::Startup {
        runtime,
        session,
        session_dir,
        cwd: cwd_str,
        resources,
        provider,
        auto,
        connect_on_start,
        models_on_start,
        resume_on_start: cli.resume,
        auth,
        state,
        env,
        settings,
        arqus_home,
    })
    .await
}

/// Load saved keys. A corrupt file is moved aside, never fatal.
pub(crate) fn load_auth(arqus_home: &std::path::Path) -> auth::AuthStore {
    let path = arqus_home.join("auth.json");
    match auth::AuthStore::load(&path) {
        Ok(store) => store,
        Err(_) => {
            auth::AuthStore::back_up_corrupt(&path);
            auth::AuthStore::default()
        }
    }
}

/// A config that lets the TUI start before a provider or model is chosen.
fn placeholder_model(provider: &str) -> ModelConfig {
    ModelConfig {
        base_url: ai::default_base_url(provider).to_string(),
        context_window: ai::default_context_window(provider),
        provider: provider.to_string(),
        model: String::new(),
        api_key: String::new(),
        extra_body: None,
    }
}

pub(crate) fn exit_config(message: String) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}

pub(crate) fn open_session(cli: &Cli, cwd: &str) -> anyhow::Result<Session> {
    if cli.no_session {
        let dir = std::env::temp_dir().join(format!("arqus-{}", new_id()));
        return Session::create(&dir, cwd);
    }
    let dir = sessions_dir(cwd);
    if cli.continue_last {
        if let Some(path) = latest_session(&dir) {
            return Session::open(&path);
        }
    }
    Session::create(&dir, cwd)
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
