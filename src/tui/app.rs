//! TUI state. Everything here is pure so it can be unit-tested without a terminal.

use std::path::PathBuf;

use crate::agent::AgentEvent;
use crate::session::{list_sessions, Session};

use super::tree::TreeState;

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
}

#[derive(Debug, Clone, Default)]
pub struct Resume {
    pub paths: Vec<PathBuf>,
    pub selected: usize,
}

/// Everything the UI draws. No ratatui types here.
pub struct App {
    pub lines: Vec<UiLine>,
    pub input: String,
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
    pub page: usize,
    pub follow: bool,
    pub resume: Option<Resume>,
    pub tree: Option<TreeState>,
    assistant_line: Option<usize>,
    thinking_line: Option<usize>,
}

impl App {
    pub fn new(session_dir: PathBuf, cwd: String, model: String, context_window: u64) -> App {
        App {
            lines: Vec::new(),
            input: String::new(),
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
            page: 10,
            follow: true,
            resume: None,
            tree: None,
            assistant_line: None,
            thinking_line: None,
        }
    }

    pub fn set_session(&mut self, session: Session) {
        self.session_file = session
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.session = Some(session);
    }

    pub fn take_input(&mut self) -> String {
        std::mem::take(&mut self.input)
    }

    pub fn push_user(&mut self, text: &str) {
        self.push(LineKind::User, text.to_string());
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.push(LineKind::Info, text.into());
    }

    pub fn clear_transcript(&mut self) {
        self.lines.clear();
        self.scroll = 0;
        self.follow = true;
        self.assistant_line = None;
        self.thinking_line = None;
    }

    pub fn set_page(&mut self, page: usize) {
        self.page = page.max(1);
        if self.follow {
            self.follow_bottom();
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.follow = false;
        self.scroll = self.scroll.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let bottom = self.lines.len().saturating_sub(self.page);
        self.scroll = (self.scroll + lines).min(bottom);
        if self.scroll == bottom {
            self.follow = true;
        }
    }

    pub fn start_resume(&mut self) -> bool {
        let paths = list_sessions(&self.session_dir);
        if paths.is_empty() {
            self.info("no sessions for this folder yet");
            return false;
        }
        self.resume = Some(Resume { paths, selected: 0 });
        true
    }

    pub fn move_resume(&mut self, delta: isize) {
        if let Some(resume) = &mut self.resume {
            let last = resume.paths.len().saturating_sub(1);
            let next = resume.selected as isize + delta;
            resume.selected = next.clamp(0, last as isize) as usize;
        }
    }

    pub fn pick_resume(&mut self) -> Option<PathBuf> {
        let resume = self.resume.take()?;
        resume.paths.get(resume.selected).cloned()
    }

    pub fn cancel_resume(&mut self) {
        self.resume = None;
    }

    /// Fold one agent event into the transcript. Pure: no drawing, no I/O.
    pub fn apply_event(&mut self, ev: AgentEvent) {
        match ev {
            AgentEvent::TextDelta(text) => {
                self.status = "streaming".to_string();
                self.thinking_line = None;
                self.assistant_line =
                    self.append_to(LineKind::Assistant, self.assistant_line, &text);
            }
            AgentEvent::ThinkingDelta(text) => {
                self.assistant_line = None;
                self.thinking_line = self.append_to(LineKind::Thinking, self.thinking_line, &text);
            }
            AgentEvent::ToolStart { name, args } => {
                self.assistant_line = None;
                self.thinking_line = None;
                let args = compact_args(&args);
                self.push(LineKind::ToolCall, format!("-> {name} {args}"));
                self.status = format!("tool: {name}");
            }
            AgentEvent::ToolEnd {
                name,
                text,
                is_error,
            } => {
                self.assistant_line = None;
                self.thinking_line = None;
                self.push_result(&name, &text, is_error);
                self.status = "waiting for model".to_string();
            }
            AgentEvent::Compacted { tokens_before } => {
                if tokens_before > 0 {
                    self.context_tokens = tokens_before;
                }
                self.push(
                    LineKind::Info,
                    format!("[compacted conversation: ~{tokens_before} tokens before]"),
                );
            }
            AgentEvent::Error(text) => {
                self.push(LineKind::Error, format!("error: {text}"));
                self.status = "error".to_string();
            }
            AgentEvent::TurnDone => {
                self.running = false;
                self.assistant_line = None;
                self.thinking_line = None;
                self.status = "ready".to_string();
            }
        }
    }

    fn push(&mut self, kind: LineKind, text: String) {
        self.lines.push(UiLine { kind, text });
        self.follow_bottom();
    }

    fn follow_bottom(&mut self) {
        if self.follow {
            self.scroll = self.lines.len().saturating_sub(self.page);
        }
    }

    fn append_to(&mut self, kind: LineKind, index: Option<usize>, text: &str) -> Option<usize> {
        if let Some(i) = index {
            if self.lines.get(i).map(|l| l.kind == kind).unwrap_or(false) {
                self.lines[i].text.push_str(text);
                self.follow_bottom();
                return Some(i);
            }
        }
        self.push(kind, text.to_string());
        self.lines.len().checked_sub(1)
    }

    fn push_result(&mut self, name: &str, text: &str, is_error: bool) {
        let kind = if is_error {
            LineKind::Error
        } else {
            LineKind::ToolResult
        };
        let head = if is_error {
            format!("<- {name} [error]: ")
        } else {
            format!("<- {name}: ")
        };
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            self.push(kind, head);
            return;
        }
        for (i, line) in lines.iter().take(10).enumerate() {
            let prefix = if i == 0 {
                head.clone()
            } else {
                "   ".to_string()
            };
            self.push(kind, format!("{prefix}{line}"));
        }
        if lines.len() > 10 {
            self.push(kind, format!("   ... {} more lines", lines.len() - 10));
        }
    }
}

fn compact_args(args: &serde_json::Value) -> String {
    let text = serde_json::to_string(args).unwrap_or_default();
    let mut out: String = text.chars().take(120).collect();
    if text.chars().count() > 120 {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
