//! Small pure facts shown in the footer and on the welcome screen.

use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ask,
    Auto,
    Plan,
}

impl Mode {
    pub fn chip(&self) -> &'static str {
        match self {
            Mode::Ask => "ask",
            Mode::Auto => "auto",
            Mode::Plan => "plan",
        }
    }

    /// Shift+Tab order: ask, auto, plan, ask.
    pub fn next(self) -> Mode {
        match self {
            Mode::Ask => Mode::Auto,
            Mode::Auto => Mode::Plan,
            Mode::Plan => Mode::Ask,
        }
    }

    /// `/mode <name>`, case-insensitive.
    pub fn parse(text: &str) -> Option<Mode> {
        match text.trim().to_ascii_lowercase().as_str() {
            "ask" => Some(Mode::Ask),
            "auto" => Some(Mode::Auto),
            "plan" => Some(Mode::Plan),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderState {
    Connected { provider: String, model: String },
    NeedsProvider,
}

/// Semantic color of one footer part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Fg,
    Dim,
    Accent,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtxLevel {
    Ok,
    Warn,
    High,
}

/// Green under 60, yellow under 85, red from 85.
pub fn ctx_level(percent: u64) -> CtxLevel {
    if percent < 60 {
        CtxLevel::Ok
    } else if percent < 85 {
        CtxLevel::Warn
    } else {
        CtxLevel::High
    }
}

/// Context use as a whole percent, never above 100.
pub fn ctx_percent(tokens: u64, window: u64) -> u64 {
    tokens
        .saturating_mul(100)
        .checked_div(window)
        .map(|p| p.min(100))
        .unwrap_or(0)
}

/// `~/code/proj` when the folder is under the home directory.
pub fn short_path(cwd: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return cwd.to_string();
    }
    if cwd == home {
        return "~".to_string();
    }
    match cwd.strip_prefix(home) {
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => cwd.to_string(),
    }
}

/// Cut a path from the left so only its tail survives: `…/code/proj`.
pub fn fit_path(path: &str, budget: usize) -> String {
    if path.width() <= budget {
        return path.to_string();
    }
    if budget <= 1 {
        return "…".to_string();
    }
    let mut tail = String::new();
    let mut used = 0;
    for ch in path.chars().rev() {
        let width = ch.to_string().width();
        if used + width > budget - 1 {
            break;
        }
        used += width;
        tail.insert(0, ch);
    }
    format!("…{tail}")
}

/// Cut the end of a string: `openrouter/deepseek…`.
pub fn truncate_end(text: &str, budget: usize) -> String {
    if text.width() <= budget {
        return text.to_string();
    }
    if budget <= 1 {
        return "…".to_string();
    }
    let mut head = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let width = ch.to_string().width();
        if used + width > budget - 1 {
            break;
        }
        used += width;
        head.push(ch);
    }
    format!("{head}…")
}

/// `$0.00` for zero, `$0.004` above it.
pub fn cost_text(cost: f64) -> String {
    if cost <= 0.0 {
        "$0.00".to_string()
    } else {
        format!("${cost:.3}")
    }
}

/// The `<provider> · <model>` line for the welcome screen.
pub fn provider_line(state: &ProviderState) -> (String, Tone) {
    match state {
        ProviderState::Connected { provider, model } => {
            (format!("{provider} · {model}"), Tone::Dim)
        }
        ProviderState::NeedsProvider => (
            "no provider connected · press ctrl+p, or type /connect".to_string(),
            Tone::Warning,
        ),
    }
}
