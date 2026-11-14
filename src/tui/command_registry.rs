//! The built-in command table and the commands discovered on disk.

use std::collections::BTreeMap;

use crate::resources::Resources;

use super::commands::{Command, CommandAction};

fn command(
    name: &str,
    aliases: &[&str],
    description: &str,
    group: &str,
    key_hint: Option<&'static str>,
    action: CommandAction,
) -> Command {
    Command {
        name: name.to_string(),
        aliases: aliases.iter().map(|alias| alias.to_string()).collect(),
        description: description.to_string(),
        group: group.to_string(),
        key_hint,
        action,
        slash: true,
    }
}

fn palette_only(
    name: &str,
    description: &str,
    group: &str,
    key: Option<&'static str>,
    action: CommandAction,
) -> Command {
    Command {
        slash: false,
        ..command(name, &[], description, group, key, action)
    }
}

/// Built-in commands of section 1.4, grouped for the palette.
pub fn registry() -> Vec<Command> {
    use CommandAction::*;
    vec![
        command(
            "connect",
            &[],
            "Add or change a provider",
            "Session",
            None,
            Connect,
        ),
        command(
            "new",
            &["clear"],
            "Start a new session",
            "Session",
            None,
            NewSession,
        ),
        command(
            "sessions",
            &["resume"],
            "List saved sessions",
            "Session",
            None,
            Sessions,
        ),
        command(
            "rename",
            &[],
            "Rename this session",
            "Session",
            None,
            Rename,
        ),
        command(
            "tree",
            &[],
            "Switch branch in the session tree",
            "Session",
            None,
            Tree,
        ),
        command(
            "fork",
            &[],
            "Branch from the current entry",
            "Session",
            None,
            Fork,
        ),
        command(
            "compact",
            &[],
            "Summarize old context",
            "Session",
            None,
            Compact,
        ),
        command("copy", &[], "Copy the last answer", "Session", None, Copy),
        command(
            "export",
            &[],
            "Export the conversation",
            "Session",
            None,
            Export,
        ),
        command(
            "mcp",
            &[],
            "List configured MCP servers",
            "Session",
            None,
            Mcp,
        ),
        command(
            "mode",
            &[],
            "ask · auto · plan",
            "Model",
            Some("shift+tab"),
            Mode,
        ),
        command(
            "models",
            &[],
            "Pick a model",
            "Model",
            Some("ctrl+l"),
            Models,
        ),
        command(
            "model",
            &[],
            "Switch model: /model provider:model",
            "Model",
            None,
            SwitchModel,
        ),
        command("theme", &[], "Change the theme", "View", None, Theme),
        command(
            "thinking",
            &[],
            "Show or hide thinking",
            "View",
            Some("ctrl+t"),
            ToggleThinking,
        ),
        command("settings", &[], "Open settings", "View", None, Settings),
        command(
            "help",
            &[],
            "List keys and commands",
            "Help",
            Some("?"),
            Help,
        ),
        command(
            "quit",
            &["exit"],
            "Quit arqus",
            "Help",
            Some("ctrl+d"),
            Quit,
        ),
        palette_only(
            "Toggle thinking",
            "Show or hide thinking",
            "View",
            Some("ctrl+t"),
            ToggleThinking,
        ),
        palette_only(
            "Toggle tool output",
            "Expand or collapse tools",
            "View",
            Some("ctrl+o"),
            ToggleToolOutput,
        ),
        palette_only(
            "Cycle mode",
            "ask → auto → plan",
            "Model",
            Some("shift+tab"),
            CycleMode,
        ),
        palette_only(
            "Cycle model",
            "Use the next recent model",
            "Model",
            Some("f2"),
            CycleModel,
        ),
        palette_only("Quit", "Quit arqus", "Help", Some("ctrl+d"), Quit),
    ]
}

/// Built-ins plus `/skill:name` and `/name` prompt templates found on disk.
pub fn with_resources(resources: &Resources) -> Vec<Command> {
    let mut commands = registry();
    for skill in &resources.skills {
        commands.push(command(
            &format!("skill:{}", skill.name),
            &[],
            &skill.description,
            "Skills",
            None,
            CommandAction::Skill(skill.name.clone()),
        ));
    }
    for name in prompt_names(resources) {
        commands.push(command(
            &name,
            &[],
            "Prompt template",
            "Prompts",
            None,
            CommandAction::Prompt(name.clone()),
        ));
    }
    commands
}

fn prompt_names(resources: &Resources) -> Vec<String> {
    let dirs = [
        resources.cwd.join(".arqus").join("prompts"),
        resources.home.join("prompts"),
    ];
    let mut names = BTreeMap::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                if valid_name(stem) {
                    names.insert(stem.to_string(), ());
                }
            }
        }
    }
    names.into_keys().collect()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
