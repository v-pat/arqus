#![allow(dead_code)] // remove this line once the modules are used by the agent

mod agent;
mod ai;
mod compaction;
mod context;
mod resources;
mod session;
mod settings;
mod system_prompt;
mod tools;
mod tree;
mod tui;
mod types;

use std::io::Write;

use anyhow::Context;
use clap::Parser;

use crate::agent::{run_prompt, AgentConfig, AgentEvent};
use crate::ai::{Llm, ModelConfig};
use crate::session::{latest_session, new_id, sessions_dir, Session};

#[derive(Debug, Parser)]
#[command(name = "pirs", version, about = "A minimal terminal coding agent")]
struct Cli {
    /// Run one prompt in print mode
    #[arg(short = 'p', long = "prompt", value_name = "PROMPT")]
    prompt: Option<String>,
    /// Continue the latest session for this directory
    #[arg(short = 'c', long = "continue")]
    continue_last: bool,
    /// Use a temporary session instead of ~/.pirs/sessions
    #[arg(long = "no-session")]
    no_session: bool,
    /// Provider and model, for example anthropic:claude-sonnet-4
    #[arg(long = "model", value_name = "PROVIDER:MODEL")]
    model: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Some(prompt) = cli.prompt.clone() {
        if let Err(err) = print_mode(&cli, prompt).await {
            eprintln!("error: {err:#}");
            std::process::exit(1);
        }
        return;
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
    let model = model_config(cli, &settings)?;
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

    let cfg = AgentConfig {
        cwd: cwd.clone(),
        system_prompt: system_prompt::build_system_prompt(
            &resources,
            &cwd_str,
            &system_prompt::today_utc(),
        ),
        context_window: model.context_window,
        compaction: settings.compaction_settings(compaction::CompactionSettings::default()),
    };
    let model_name = if model.model.is_empty() {
        "(no model)".to_string()
    } else {
        model.model.clone()
    };
    let runtime = agent::Runtime {
        llm: Llm::from_config(model),
        cfg,
        model_name,
    };

    tui::run(tui::Startup {
        runtime,
        session,
        session_dir,
        cwd: cwd_str,
        resources,
    })
    .await
}

async fn print_mode(cli: &Cli, prompt: String) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("cannot read current directory")?;
    let cwd_str = cwd.to_string_lossy().to_string();

    let settings = settings::Settings::load(&cwd);
    let model = model_config(cli, &settings)?;
    let resources = resources::load_with(&cwd, &settings.skill_folders);
    let mut session = open_session(cli, &cwd_str)?;
    let cfg = AgentConfig {
        cwd: cwd.clone(),
        system_prompt: system_prompt::build_system_prompt(
            &resources,
            &cwd_str,
            &system_prompt::today_utc(),
        ),
        context_window: model.context_window,
        compaction: settings.compaction_settings(compaction::CompactionSettings::default()),
    };
    let llm = Llm::from_config(model);

    if let Some(rest) = prompt.strip_prefix("/compact") {
        let custom = rest.trim();
        let custom = if custom.is_empty() {
            None
        } else {
            Some(custom)
        };
        let done = compaction::compact(&mut session, &llm, &cfg.compaction, custom).await?;
        if done {
            println!("compacted");
        } else {
            eprintln!("nothing to compact");
        }
        return Ok(());
    }

    if prompt == "/tree" || prompt.starts_with("/tree ") {
        let id = prompt.strip_prefix("/tree").unwrap_or("").trim();
        if id.is_empty() {
            for row in tree::rows(&session.entries, session.leaf_id.as_deref()) {
                let marker = if row.on_path { "*" } else { " " };
                let indent = "  ".repeat(row.depth);
                println!("{marker} {indent}{}: {}", row.role, row.preview);
            }
        } else {
            session
                .set_leaf(id)
                .with_context(|| format!("cannot move to entry {id}"))?;
            println!(
                "path: {}",
                tree::path_text(&session.entries, session.leaf_id.as_deref())
            );
        }
        return Ok(());
    }

    if prompt == "/fork" {
        let dir = session
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| sessions_dir(&cwd_str));
        let forked = tree::fork(&session, &dir)?;
        println!("forked to {}", forked.path.display());
        return Ok(());
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let prompt = resources::expand_input(&resources, &prompt);
    let printer = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            print_event(event);
        }
    });
    let result = run_prompt(&mut session, &llm, &cfg, prompt, tx).await;
    printer.await.ok();
    result
}

fn model_config(cli: &Cli, settings: &settings::Settings) -> anyhow::Result<ModelConfig> {
    settings::EnvOverrides::from_env().resolve(settings, cli.model.as_deref())
}

fn open_session(cli: &Cli, cwd: &str) -> anyhow::Result<Session> {
    if cli.no_session {
        let dir = std::env::temp_dir().join(format!("pirs-{}", new_id()));
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

/// Print mode: assistant text on stdout, short tool lines on stderr.
fn print_event(event: AgentEvent) {
    match event {
        AgentEvent::TextDelta(text) => {
            print!("{text}");
            std::io::stdout().flush().ok();
        }
        AgentEvent::ThinkingDelta(_) => {}
        AgentEvent::ToolStart { name, args } => eprintln!("-> {name} {args}"),
        AgentEvent::ToolEnd {
            name,
            text,
            is_error,
        } => {
            let status = if is_error { "error" } else { "ok" };
            eprintln!("<- {name} [{status}]: {}", cut(&text, 200));
        }
        AgentEvent::Compacted { tokens_before } => {
            eprintln!("[compacted conversation: ~{tokens_before} tokens before]")
        }
        AgentEvent::TurnDone => println!(),
        AgentEvent::Error(text) => eprintln!("error: {text}"),
    }
}

fn cut(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
