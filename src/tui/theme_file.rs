//! Theme files in `<ARQUS_HOME>/themes/<name>.json`.
//!
//! Every color is a `#rrggbb` string. Missing fields use the dark defaults, so
//! a small file can override one color. Invalid files are skipped.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::theme::{ColorDepth, Palette, Theme};

pub const BUILTINS: [&str; 4] = ["dark", "light", "mono", "dark (256 colors)"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedTheme {
    pub name: String,
    pub theme: Theme,
}

/// One built-in theme by name, at `depth`.
pub fn builtin(name: &str, depth: ColorDepth) -> Option<Theme> {
    match name {
        "dark" => Some(Theme::dark(depth)),
        "light" => Some(Theme::light(depth)),
        "mono" => Some(Theme::mono()),
        "dark (256 colors)" => Some(Theme::dark(ColorDepth::Indexed256)),
        _ => None,
    }
}

/// A saved name: built-in first, then `<home>/themes/<name>.json`.
pub fn load_named(home: &Path, name: &str, depth: ColorDepth) -> Option<Theme> {
    if let Some(theme) = builtin(name, depth) {
        return Some(theme);
    }
    let path = theme_path(home, name)?;
    let text = fs::read_to_string(path).ok()?;
    parse(name, &text, depth).ok()
}

/// Built-ins plus every valid theme file, in file-name order.
pub fn all(home: &Path, depth: ColorDepth) -> (Vec<NamedTheme>, Vec<String>) {
    let mut choices: Vec<NamedTheme> = BUILTINS
        .iter()
        .filter_map(|name| {
            builtin(name, depth).map(|theme| NamedTheme {
                name: (*name).to_string(),
                theme,
            })
        })
        .collect();
    let mut errors = Vec::new();
    let dir = home.join("themes");
    let Ok(entries) = fs::read_dir(&dir) else {
        return (choices, errors);
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    files.sort();
    for path in files {
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let name = stem.to_string();
        let display = format!("themes/{stem}.json");
        match fs::read_to_string(&path) {
            Ok(text) => match parse(&name, &text, depth) {
                Ok(theme) => choices.push(NamedTheme { name, theme }),
                Err(err) => errors.push(format!("{display}: {err}")),
            },
            Err(err) => errors.push(format!("{display}: {err}")),
        }
    }
    (choices, errors)
}

/// A theme for startup. `NO_COLOR` wins over any saved name.
pub fn startup_theme(home: &Path, saved: Option<&str>, depth: ColorDepth) -> Theme {
    if depth == ColorDepth::None {
        return Theme::mono();
    }
    saved
        .and_then(|name| load_named(home, name, depth))
        .unwrap_or_else(|| Theme::dark(depth))
}

fn theme_path(home: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return None;
    }
    Some(home.join("themes").join(format!("{name}.json")))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ThemeFile {
    fg: Option<String>,
    dim: Option<String>,
    accent: Option<String>,
    user: Option<String>,
    assistant: Option<String>,
    thinking: Option<String>,
    tool: Option<String>,
    success: Option<String>,
    warning: Option<String>,
    error: Option<String>,
    border: Option<String>,
    border_focus: Option<String>,
    selection: Option<String>,
    diff_add: Option<String>,
    diff_del: Option<String>,
    code_bg: Option<String>,
}

/// Parse one theme file. Missing colors fall back to the dark palette.
pub fn parse(name: &str, text: &str, depth: ColorDepth) -> Result<Theme, String> {
    let file: ThemeFile = serde_json::from_str(text).map_err(|err| err.to_string())?;
    let mut palette = Palette::dark();
    set(&mut palette.fg, &file.fg, "fg")?;
    set(&mut palette.dim, &file.dim, "dim")?;
    set(&mut palette.accent, &file.accent, "accent")?;
    set(&mut palette.user, &file.user, "user")?;
    set(&mut palette.assistant, &file.assistant, "assistant")?;
    set(&mut palette.thinking, &file.thinking, "thinking")?;
    set(&mut palette.tool, &file.tool, "tool")?;
    set(&mut palette.success, &file.success, "success")?;
    set(&mut palette.warning, &file.warning, "warning")?;
    set(&mut palette.error, &file.error, "error")?;
    set(&mut palette.border, &file.border, "border")?;
    set(
        &mut palette.border_focus,
        &file.border_focus,
        "border_focus",
    )?;
    set(&mut palette.selection, &file.selection, "selection")?;
    set(&mut palette.diff_add, &file.diff_add, "diff_add")?;
    set(&mut palette.diff_del, &file.diff_del, "diff_del")?;
    set(&mut palette.code_bg, &file.code_bg, "code_bg")?;
    Ok(Theme::build(name, depth, &palette))
}

fn set(slot: &mut (u8, u8, u8), value: &Option<String>, field: &str) -> Result<(), String> {
    if let Some(text) = value {
        *slot = parse_hex(text).map_err(|err| format!("{field}: {err}"))?;
    }
    Ok(())
}

fn parse_hex(text: &str) -> Result<(u8, u8, u8), String> {
    let hex = text.trim();
    let digits = hex
        .strip_prefix('#')
        .ok_or_else(|| format!("\"{hex}\" is not #rrggbb"))?;
    if digits.len() != 6 || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("\"{hex}\" is not #rrggbb"));
    }
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&digits[range], 16).map_err(|err| err.to_string())
    };
    Ok((channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

#[cfg(test)]
#[path = "theme_file_tests.rs"]
mod tests;
