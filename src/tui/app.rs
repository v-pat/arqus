//! TUI state. Everything here is pure so it can be unit-tested without a terminal.

use std::collections::VecDeque;
use std::path::PathBuf;

use ratatui::text::Line;

use super::approval::Approval;
use super::block::{self, Block};
use super::commands::{self, Command};
use super::footer::{Mode, ProviderState};
use super::git::GitInfo;
use super::input::InputState;
use super::keys::KeyMap;
use super::msg::Level;
use super::overlay::Overlay;
use super::picker::Picker;
use super::render::BlockCache;
use super::resume::Resume;
use super::slash::SlashState;
use super::theme::{ColorDepth, Theme};
use super::toast::Toast;
use super::tree::TreeState;
use crate::auth::AuthStore;
use crate::safety::Decision;
use crate::session::Session;
use crate::settings::{EnvOverrides, Settings};
use crate::state::State;

/// What decides whether a cached block render is still valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheKey {
    pub width: usize,
    pub content: usize,
    pub expanded: bool,
    pub show_thinking: bool,
}

/// Everything the UI draws. No terminal types except the render cache.
pub struct App {
    /// Pending approval, drawn as a modal over everything else.
    pub approval: Option<Approval>,
    /// Reply channel for the running agent.
    pub approval_tx: Option<tokio::sync::mpsc::Sender<Decision>>,
    /// Transcript blocks, as the agent emits them.
    pub blocks: Vec<Block>,
    /// One render cache per block, keyed by width, content and collapse flags.
    pub cache: Vec<BlockCache>,
    /// Flattened render output, ready to draw.
    pub wrapped: Vec<Line<'static>>,
    pub wrap_width: usize,
    /// `Ctrl+O`: show every tool output up to 200 lines.
    pub expanded: bool,
    /// `Ctrl+T`: show thinking text instead of one dim line.
    pub show_thinking: bool,
    pub input: InputState,
    /// Prompts typed while a run is active, not yet accepted by the agent.
    pub queue: Vec<String>,
    /// Project files for `@` mentions, filled by `Effect::ScanFiles`.
    pub files: Vec<String>,
    /// The `@` popup, when the word at the cursor starts with `@`.
    pub mention: Option<Picker<String>>,
    /// Char index where the `@` word in the input begins.
    pub mention_start: usize,
    /// Input text for which Esc closed the `@` popup.
    pub mention_dismissed: Option<String>,
    /// Show the Mac Option-as-Meta note only once.
    pub mac_note_shown: bool,
    /// Sender to the running turn. `None` when no run is active.
    pub steer_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    /// Tick of the first idle Ctrl+C, until the 2 second confirm window ends.
    pub ctrl_c_tick: Option<u64>,
    /// Tick of the first idle Esc, until the rewind window ends.
    pub esc_tick: Option<u64>,
    pub scroll: usize,
    pub running: bool,
    pub status: String,
    pub session: Option<Session>,
    /// Title from the last `session_name` entry, or the first user message.
    pub session_title: String,
    pub session_dir: PathBuf,
    pub session_file: String,
    pub cwd: String,
    /// Home directory, shown as `~` in the footer.
    pub home: String,
    pub model: String,
    /// Which provider the TUI is ready to talk to.
    pub provider_state: ProviderState,
    /// Footer chip: ask, auto or plan.
    pub mode: Mode,
    /// True once the auto warning toast has been shown in this session.
    pub auto_warned: bool,
    /// Tick of the last key or paste, so the bell only rings after 10 idle seconds.
    pub last_input_tick: u64,
    /// Branch and dirty flag, when the project is a git repository.
    pub git: Option<GitInfo>,
    /// Transient messages at the top right.
    pub toasts: VecDeque<Toast>,
    /// `Retrying in 4s (attempt 2/4)...` while the engine backs off.
    pub retry_notice: Option<String>,
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
    /// Built-ins plus `/skill:name` and `/name` prompt templates.
    pub commands: Vec<Command>,
    /// The `/` popup state, recomputed from the input after every key.
    pub slash: SlashState,
    /// Set when something changed; the event loop redraws only then.
    pub dirty: bool,
    /// Saved API keys, loaded from `<ARQUS_HOME>/auth.json`.
    pub auth: AuthStore,
    /// Last model, recent models, theme and base URL overrides.
    pub state: State,
    /// Environment snapshot, so key resolution is pure.
    pub env: EnvOverrides,
    pub settings: Settings,
    pub arqus_home: PathBuf,
}

impl App {
    pub fn new(session_dir: PathBuf, cwd: String, model: String, context_window: u64) -> App {
        App {
            approval: None,
            approval_tx: None,
            blocks: Vec::new(),
            cache: Vec::new(),
            wrapped: Vec::new(),
            wrap_width: 0,
            expanded: false,
            show_thinking: false,
            input: InputState::new(),
            queue: Vec::new(),
            files: Vec::new(),
            mention: None,
            mention_start: 0,
            mention_dismissed: None,
            mac_note_shown: false,
            steer_tx: None,
            ctrl_c_tick: None,
            esc_tick: None,
            scroll: 0,
            running: false,
            status: "ready".to_string(),
            session: None,
            session_title: String::new(),
            session_dir,
            session_file: String::new(),
            cwd,
            home: dirs::home_dir()
                .map(|home| home.to_string_lossy().to_string())
                .unwrap_or_default(),
            model,
            provider_state: ProviderState::NeedsProvider,
            mode: Mode::Ask,
            auto_warned: false,
            last_input_tick: 0,
            git: None,
            toasts: VecDeque::new(),
            retry_notice: None,
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
            commands: commands::registry(),
            slash: SlashState::default(),
            dirty: true,
            auth: AuthStore::default(),
            state: State::default(),
            env: EnvOverrides::default(),
            settings: Settings::default(),
            arqus_home: crate::home::arqus_home(),
        }
    }

    pub fn set_size(&mut self, width: u16, height: u16) {
        let rewrap = width as usize != self.wrap_width;
        let anchor = if rewrap && !self.follow {
            self.top_anchor()
        } else {
            None
        };
        self.set_page(height.saturating_sub(5) as usize);
        self.set_width(width as usize);
        if let Some(anchor) = anchor {
            self.scroll_to_anchor(anchor);
        }
    }

    pub fn push_overlay(&mut self, overlay: Overlay) {
        self.overlays.push(overlay);
        self.dirty = true;
    }

    pub fn pop_overlay(&mut self) -> Option<Overlay> {
        self.dirty = true;
        self.overlays.pop()
    }

    pub fn set_session(&mut self, session: Session) {
        self.session_file = session
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.session_cost = crate::cost::session_totals(&session.entries).cost;
        self.session_title = crate::session::title(&session.entries);
        self.session = Some(session);
    }

    /// Rebuild the transcript from the current path, for a resume or a leaf move.
    pub fn replay_session(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let path: Vec<crate::types::Entry> =
            crate::session::path_to_root(&session.entries, session.leaf_id.as_deref())
                .into_iter()
                .cloned()
                .collect();
        self.blocks = block::blocks_from_entries(&path);
        self.cache.clear();
        self.refresh_render();
        self.end_follow();
    }

    pub fn take_input(&mut self) -> String {
        self.input.take()
    }

    /// Remember that the user pressed a key or pasted text.
    pub fn mark_activity(&mut self) {
        self.last_input_tick = self.ticks;
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

    /// Pop every queued prompt, newest last, joined by newlines.
    pub fn take_queue_text(&mut self) -> Option<String> {
        if self.queue.is_empty() {
            return None;
        }
        let text = self.queue.join("\n");
        self.queue.clear();
        Some(text)
    }

    /// Take the last queued prompt back into the editor.
    pub fn recall_queued(&mut self) -> bool {
        let Some(item) = self.queue.pop() else {
            return false;
        };
        let current = self.input.text();
        if current.trim().is_empty() {
            self.input.set_text(&item);
        } else {
            self.input.set_text(&format!("{current}\n{item}"));
        }
        true
    }

    /// Start a local `!command` card before the command runs.
    pub fn start_local_tool(&mut self, command: &str) -> usize {
        let args = serde_json::json!({ "command": command });
        let id = self.blocks.len();
        self.blocks
            .push(Block::Tool(super::block::ToolBlock::new(id, "bash", &args)));
        self.refresh_render();
        self.follow_bottom();
        id
    }

    /// Finish the local command card and record one user message.
    pub fn finish_local_tool(&mut self, command: &str, output: &str, is_error: bool) {
        if let Some(Block::Tool(tool)) = self
            .blocks
            .iter_mut()
            .rev()
            .find(|block| matches!(block, Block::Tool(tool) if tool.state == super::block::ToolState::Running))
        {
            tool.finish(output.to_string(), is_error);
        }
        self.refresh_render();
        self.follow_bottom();
        if let Some(session) = self.session.as_mut() {
            let text = super::shell::message(command, output);
            let _ = session.append(crate::types::EntryKind::Message {
                message: crate::types::Message::User { text },
            });
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

    /// Esc on an empty idle box. Returns true on the second press inside the
    /// 2 second window; the first press only shows the hint.
    pub fn esc_idle(&mut self) -> bool {
        const WINDOW_TICKS: u64 = 20;
        if let Some(tick) = self.esc_tick {
            if self.ticks.saturating_sub(tick) <= WINDOW_TICKS {
                self.esc_tick = None;
                return true;
            }
        }
        self.esc_tick = Some(self.ticks);
        self.status = "press Esc again to open the tree".to_string();
        false
    }

    pub fn push_block(&mut self, block: Block) {
        self.blocks.push(block);
        self.refresh_render();
        self.follow_bottom();
    }

    pub fn push_user(&mut self, text: &str) {
        self.push_block(Block::User(text.to_string()));
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.push_block(Block::Notice(text.into(), Level::Info));
    }

    pub fn clear_transcript(&mut self) {
        self.blocks.clear();
        self.cache.clear();
        self.wrapped.clear();
        self.scroll = 0;
        self.follow = true;
    }

    /// True once a user, model or tool block exists. Notices and compaction
    /// lines alone keep the welcome screen visible.
    pub fn has_conversation(&self) -> bool {
        self.blocks
            .iter()
            .any(|block| !matches!(block, Block::Notice(..) | Block::Compaction { .. }))
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "app_session_tests.rs"]
mod session_tests;
