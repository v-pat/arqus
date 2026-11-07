//! TUI state. Everything here is pure so it can be unit-tested without a terminal.

use std::path::PathBuf;

use super::approval::Approval;
use super::input::InputState;
use super::keys::KeyMap;
use super::msg::Level;
use super::overlay::Overlay;
use super::resume::Resume;
use super::theme::{ColorDepth, Theme};
use super::tree::TreeState;
use super::wrap::build_wrapped;
use crate::safety::Decision;
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    User,
    Assistant,
    Thinking,
    ToolCall,
    ToolResult,
    Error,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiLine {
    pub kind: LineKind,
    pub text: String,
    /// Dim style: draw lines inside a fenced code block.
    pub dim: bool,
}

/// Everything the UI draws. No ratatui types here.
pub struct App {
    /// Pending approval, drawn as a modal over everything else.
    pub approval: Option<Approval>,
    /// Reply channel for the running agent.
    pub approval_tx: Option<tokio::sync::mpsc::Sender<Decision>>,
    /// Logical lines, as the agent emits them.
    pub lines: Vec<UiLine>,
    /// `lines` after wrapping to `wrap_width`, ready to draw.
    pub wrapped: Vec<UiLine>,
    pub wrap_width: usize,
    pub input: InputState,
    /// Prompts typed while a run is active, not yet accepted by the agent.
    pub queue: Vec<String>,
    /// Sender to the running turn. `None` when no run is active.
    pub steer_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    /// Tick of the first idle Ctrl+C, until the 2 second confirm window ends.
    pub ctrl_c_tick: Option<u64>,
    pub scroll: usize,
    pub running: bool,
    pub status: String,
    pub session: Option<Session>,
    pub session_dir: PathBuf,
    pub session_file: String,
    pub cwd: String,
    pub model: String,
    pub context_tokens: u64,
    pub context_window: u64,
    /// Session cost in dollars, when any request reported one.
    pub session_cost: Option<f64>,
    pub page: usize,
    pub follow: bool,
    /// 100 ms ticks since startup, for the spinner and elapsed time.
    pub ticks: u64,
    pub elapsed_ticks: u64,
    pub resume: Option<Resume>,
    pub tree: Option<TreeState>,
    /// Dialogs drawn over the screen; the last one owns the keyboard.
    pub overlays: Vec<Overlay>,
    pub theme: Theme,
    pub keymap: KeyMap,
    /// Set when something changed; the event loop redraws only then.
    pub dirty: bool,
    pub(crate) assistant_line: Option<usize>,
    pub(crate) thinking_line: Option<usize>,
}

impl App {
    pub fn new(session_dir: PathBuf, cwd: String, model: String, context_window: u64) -> App {
        App {
            approval: None,
            approval_tx: None,
            lines: Vec::new(),
            wrapped: Vec::new(),
            wrap_width: 0,
            input: InputState::new(),
            queue: Vec::new(),
            steer_tx: None,
            ctrl_c_tick: None,
            scroll: 0,
            running: false,
            status: "ready".to_string(),
            session: None,
            session_dir,
            session_file: String::new(),
            cwd,
            model,
            context_tokens: 0,
            context_window,
            session_cost: None,
            page: 10,
            follow: true,
            ticks: 0,
            elapsed_ticks: 0,
            resume: None,
            tree: None,
            overlays: Vec::new(),
            theme: Theme::dark(ColorDepth::True),
            keymap: KeyMap::defaults(),
            dirty: true,
            assistant_line: None,
            thinking_line: None,
        }
    }

    /// Feed the terminal size to the caches that depend on it.
    pub fn set_size(&mut self, width: u16, height: u16) {
        self.set_page(height.saturating_sub(5) as usize);
        self.set_width(width as usize);
    }

    pub fn push_overlay(&mut self, overlay: Overlay) {
        self.overlays.push(overlay);
        self.dirty = true;
    }

    pub fn pop_overlay(&mut self) -> Option<Overlay> {
        self.dirty = true;
        self.overlays.pop()
    }

    pub fn toast(&mut self, text: impl Into<String>, level: Level) {
        self.status = text.into();
        if level == Level::Error {
            self.dirty = true;
        }
    }

    pub fn set_session(&mut self, session: Session) {
        self.session_file = session
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.session_cost = crate::cost::session_totals(&session.entries).cost;
        self.session = Some(session);
    }

    pub fn take_input(&mut self) -> String {
        self.input.take()
    }

    /// Save a submitted prompt in the history file.
    pub fn remember_prompt(&mut self, text: &str) {
        self.input.remember(text);
    }

    /// Queue a prompt typed during a run. False when no run is listening.
    pub fn queue_prompt(&mut self, text: &str) -> bool {
        let sent = match &self.steer_tx {
            Some(tx) => tx.send(text.to_string()).is_ok(),
            None => false,
        };
        if sent {
            self.queue.push(text.to_string());
        }
        sent
    }

    /// Drop every queued prompt. Returns how many were dropped.
    pub fn clear_queue(&mut self) -> usize {
        let count = self.queue.len();
        self.queue.clear();
        count
    }

    pub(crate) fn dequeue(&mut self, text: &str) {
        if let Some(pos) = self.queue.iter().position(|queued| queued == text) {
            self.queue.remove(pos);
        }
    }

    /// Ctrl+C with an empty box. Returns true on the second press inside the
    /// 2 second window; the first press only shows the warning.
    pub fn ctrl_c_idle(&mut self) -> bool {
        const WINDOW_TICKS: u64 = 20;
        if let Some(tick) = self.ctrl_c_tick {
            if self.ticks.saturating_sub(tick) <= WINDOW_TICKS {
                return true;
            }
        }
        self.ctrl_c_tick = Some(self.ticks);
        self.status = "press Ctrl+C again to quit".to_string();
        false
    }

    pub fn push_user(&mut self, text: &str) {
        self.push(LineKind::User, text.to_string());
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.push(LineKind::Info, text.into());
    }

    pub fn clear_transcript(&mut self) {
        self.lines.clear();
        self.wrapped.clear();
        self.scroll = 0;
        self.follow = true;
        self.assistant_line = None;
        self.thinking_line = None;
    }

    /// Rebuild the wrapped view from the logical lines. Cheap enough for a
    /// whole transcript; only called when the lines or the width change.
    pub fn refresh_wrap(&mut self) {
        self.wrapped = build_wrapped(&self.lines, self.wrap_width);
    }
}

/// Text as drawn, including the role prefix used by the transcript.
pub fn display_text(line: &UiLine) -> String {
    match line.kind {
        LineKind::User => format!("> {}", line.text),
        LineKind::Thinking => format!("  {}", line.text),
        _ => line.text.clone(),
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "app_session_tests.rs"]
mod session_tests;
