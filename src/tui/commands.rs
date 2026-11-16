//! One command table used by the `/` menu, the `Ctrl+P` palette and `?` help.
//!
//! `run` is pure: it changes `App` and returns `Effect` values. Actions that
//! arrive in a later stage answer with the `coming in a later stage` toast.

use super::app::App;
use super::footer::Mode;
use super::keys::Action as KeyAction;
use super::msg::{Effect, Level};
use super::overlay::Overlay;

pub use super::command_registry::{registry, with_resources};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    NotYet,
    Help,
    NewSession,
    Sessions,
    Tree,
    Fork,
    Compact,
    Cost,
    Budget,
    Mode,
    SwitchModel,
    Quit,
    Connect,
    Models,
    Rename,
    Theme,
    Settings,
    ToggleThinking,
    ToggleToolOutput,
    CycleMode,
    CycleModel,
    Copy,
    Export,
    Mcp,
    Prompt(String),
    Skill(String),
}

impl CommandAction {
    /// True when the action does something today (help hides the rest).
    pub fn available(&self) -> bool {
        !matches!(self, CommandAction::NotYet)
    }

    /// Key map action that should be shown as this command's hint.
    pub fn key_action(&self) -> Option<KeyAction> {
        Some(match self {
            CommandAction::Help => KeyAction::Help,
            CommandAction::Models => KeyAction::Models,
            CommandAction::Mode | CommandAction::CycleMode => KeyAction::CycleMode,
            CommandAction::SwitchModel | CommandAction::CycleModel => KeyAction::CycleModel,
            CommandAction::ToggleThinking => KeyAction::ToggleThinking,
            CommandAction::ToggleToolOutput => KeyAction::ToggleToolOutput,
            CommandAction::Quit => KeyAction::CtrlD,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Slash name without the `/`; palette-only rows use a plain title.
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub group: String,
    pub key_hint: Option<&'static str>,
    pub action: CommandAction,
    /// True when the slash menu may offer this command.
    pub slash: bool,
}

impl Command {
    pub fn label(&self) -> String {
        if self.slash {
            format!("/{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Run the text typed after `Enter`. Unknown slash words stay plain prompts.
pub fn dispatch(app: &mut App, text: &str) -> Vec<Effect> {
    let trimmed = text.trim();
    let body = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let (word, rest) = match body.find(char::is_whitespace) {
        Some(index) => (&body[..index], body[index..].trim()),
        None => (body, ""),
    };
    let found = app
        .commands
        .iter()
        .find(|candidate| {
            candidate.slash
                && (candidate.name == word || candidate.aliases.iter().any(|alias| alias == word))
        })
        .cloned();
    match found {
        Some(command) => run(app, &command, rest),
        None => run_prompt(app, text),
    }
}

/// Run one command. `args` is the text after its name (may be empty).
pub fn run(app: &mut App, command: &Command, args: &str) -> Vec<Effect> {
    match &command.action {
        CommandAction::NotYet => {
            not_yet(app);
            Vec::new()
        }
        CommandAction::Theme => super::flow_settings::open_theme(app),
        CommandAction::Settings => super::flow_settings::open_settings(app),
        CommandAction::CycleMode => cycle_mode(app),
        CommandAction::Rename => {
            rename(app, args);
            Vec::new()
        }
        CommandAction::Copy => {
            if args.trim() == "code" {
                return match last_code_block(&app.blocks) {
                    Some(text) => vec![Effect::Copy(text)],
                    None => {
                        app.toast("no code block in the last answer", Level::Warn);
                        Vec::new()
                    }
                };
            }
            match last_assistant(&app.blocks) {
                Some(text) => vec![Effect::Copy(text)],
                None => {
                    app.toast("nothing to copy", Level::Warn);
                    Vec::new()
                }
            }
        }
        CommandAction::Export => {
            let path = args.trim();
            vec![Effect::Export(if path.is_empty() {
                None
            } else {
                Some(path.to_string())
            })]
        }
        CommandAction::Mcp => {
            mcp_info(app);
            Vec::new()
        }
        CommandAction::ToggleThinking => {
            app.toggle_thinking();
            app.state.show_thinking = app.show_thinking;
            vec![Effect::SaveState]
        }
        CommandAction::ToggleToolOutput => {
            app.toggle_expanded();
            Vec::new()
        }
        CommandAction::Connect => {
            app.push_overlay(Overlay::Connect(super::overlay::connect::Connect::new(
                None,
                app.connect_ctx(),
            )));
            Vec::new()
        }
        CommandAction::CycleModel => super::flow::cycle_model(app),
        CommandAction::Help => {
            app.push_overlay(Overlay::Help);
            Vec::new()
        }
        CommandAction::NewSession => vec![Effect::NewSession],
        CommandAction::Sessions => vec![Effect::LoadSessions],
        CommandAction::Tree => {
            app.start_tree();
            Vec::new()
        }
        CommandAction::Fork => vec![Effect::Fork],
        CommandAction::Compact => {
            let focus = args.trim();
            let label = if focus.is_empty() {
                "/compact".to_string()
            } else {
                format!("/compact {focus}")
            };
            app.push_user(&label);
            app.running = true;
            app.elapsed_ticks = 0;
            app.status = "compacting".to_string();
            let focus = if focus.is_empty() {
                None
            } else {
                Some(focus.to_string())
            };
            vec![Effect::Compact(focus)]
        }
        CommandAction::Cost => {
            let line = match app.session.as_ref() {
                Some(session) => {
                    crate::cost::totals_line(&crate::cost::session_totals(&session.entries))
                }
                None => "no session yet".to_string(),
            };
            app.info(line);
            Vec::new()
        }
        CommandAction::Budget => {
            app.info("checking budget...");
            vec![Effect::Budget]
        }
        CommandAction::Mode => mode_command(app, args),
        CommandAction::SwitchModel => {
            if args.trim().is_empty() {
                app.info(format!("model: {}", app.model));
                Vec::new()
            } else {
                vec![Effect::SwitchModel(args.trim().to_string())]
            }
        }
        CommandAction::Quit => vec![Effect::Quit],
        CommandAction::Models => super::flow::open_models(app),
        CommandAction::Prompt(_) | CommandAction::Skill(_) => {
            run_prompt(app, &format!("/{}", command.name))
        }
    }
}

fn rename(app: &mut App, args: &str) {
    let name = args.trim();
    if name.is_empty() {
        app.toast("usage: /rename <name>", Level::Warn);
        return;
    }
    let result = match app.session.as_mut() {
        Some(session) => session.append(crate::types::EntryKind::Custom {
            custom_type: "session_name".to_string(),
            data: serde_json::json!({ "name": name }),
        }),
        None => {
            app.toast("no session: use /new", Level::Warn);
            return;
        }
    };
    match result {
        Ok(_) => {
            app.session_title = name.to_string();
            app.toast(format!("Session renamed to {name}"), Level::Info);
        }
        Err(err) => app.toast(format!("cannot rename: {err:#}"), Level::Error),
    }
}

fn mcp_info(app: &mut App) {
    let mcp_cfg = crate::mcp::config::McpConfig::load(std::path::Path::new(&app.cwd));
    if mcp_cfg.servers.is_empty() {
        app.toast("no MCP servers configured", Level::Info);
        app.info("No MCP servers configured in mcp.json.");
    } else {
        let count = mcp_cfg.servers.len();
        let mut msg = format!("Configured MCP servers ({count}):\n");
        for (name, conf) in &mcp_cfg.servers {
            msg.push_str(&format!(
                "  • {name}: {} {}\n",
                conf.command,
                conf.args.join(" ")
            ));
        }
        app.info(msg.trim_end().to_string());
        app.toast(format!("{count} MCP server(s) configured"), Level::Info);
    }
}

fn last_assistant(blocks: &[super::block::Block]) -> Option<String> {
    blocks.iter().rev().find_map(|block| match block {
        super::block::Block::Assistant { text, .. } if !text.trim().is_empty() => {
            Some(text.clone())
        }
        _ => None,
    })
}

/// Last fenced code block of the last answer, without the fences.
fn last_code_block(blocks: &[super::block::Block]) -> Option<String> {
    let text = blocks.iter().rev().find_map(|block| match block {
        super::block::Block::Assistant { text, .. } if !text.trim().is_empty() => {
            Some(text.as_str())
        }
        _ => None,
    })?;
    let mut current: Option<Vec<&str>> = None;
    let mut last: Option<String> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match current.take() {
                Some(lines) => last = Some(lines.join("\n")),
                None => current = Some(Vec::new()),
            }
            continue;
        }
        if let Some(lines) = current.as_mut() {
            lines.push(line);
        }
    }
    if let Some(lines) = current {
        last = Some(lines.join("\n"));
    }
    last.filter(|block| !block.trim().is_empty())
}

fn run_prompt(app: &mut App, text: &str) -> Vec<Effect> {
    app.push_user(text);
    app.running = true;
    app.elapsed_ticks = 0;
    app.status = "running".to_string();
    vec![Effect::RunPrompt(text.to_string())]
}

pub const AUTO_WARNING: &str =
    "auto: tools run without asking. Use a git branch or the Docker sandbox.";
pub const NANOFLOW_INFO: &str =
    "nanoflow: collaborative multi-model planning and execution (experimental)";

/// Switch mode and tell the engine. The choice is never written to state.json.
fn mode_command(app: &mut App, args: &str) -> Vec<Effect> {
    if args.trim().is_empty() {
        return cycle_mode(app);
    }
    match Mode::parse(args) {
        Some(mode) => set_mode(app, mode),
        None => {
            app.toast("mode: use ask, auto, plan, or nanoflow", Level::Warn);
            Vec::new()
        }
    }
}

/// Shift+Tab: ask -> auto -> plan -> nanoflow -> ask.
pub fn cycle_mode(app: &mut App) -> Vec<Effect> {
    set_mode(app, app.mode.next())
}

/// Apply one mode, with the first-use auto warning.
pub fn set_mode(app: &mut App, mode: Mode) -> Vec<Effect> {
    app.mode = mode;
    app.toast(format!("mode: {}", mode.chip()), Level::Info);
    if mode == Mode::Auto && !app.auto_warned {
        app.auto_warned = true;
        app.toast(AUTO_WARNING, Level::Warn);
    } else if mode == Mode::Nanoflow && !app.nanoflow_warned {
        app.nanoflow_warned = true;
        app.toast(NANOFLOW_INFO, Level::Info);
    }
    vec![Effect::SetMode(mode)]
}

pub fn not_yet(app: &mut App) {
    app.toast("coming in a later stage", Level::Info);
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
