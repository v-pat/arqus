//! Everything that can happen (`Msg`) and everything the world must do
//! (`Effect`). `update()` is the only function that turns one into the other.

use std::path::PathBuf;

use crate::agent::AgentEvent;
use crate::models::ModelInfo;
use crate::session::Session;

use super::footer::Mode;
use super::keys::Key;
use super::overlay::connect::{KeyError, KeyInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

pub enum Msg {
    Key(Key),
    Paste(String),
    Resize(u16, u16),
    /// Mouse wheel: negative up, positive down.
    Wheel(i32),
    /// SIGTERM, SIGHUP or a closed stdin stream.
    Signal,
    Tick,
    Started,
    Agent(AgentEvent),
    Done(Session),
    Toast(String, Level),
    Git {
        branch: Option<String>,
        dirty: bool,
    },
    Sessions(Vec<super::sessions::SessionInfo>),
    Opened {
        session: Session,
        status: String,
    },
    KeyChecked(Result<KeyInfo, KeyError>),
    ModelsLoaded(Result<Vec<ModelInfo>, String>),
    ModelChanged {
        provider: String,
        model: String,
    },
    /// Project files for `@` mentions.
    Files(Vec<String>),
    /// A `!command` finished, with the bash tool's output.
    LocalDone {
        command: String,
        text: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    RunPrompt(String),
    Compact(Option<String>),
    Abort,
    Quit,
    Fork,
    NewSession,
    SwitchModel(String),
    Budget,
    LoadSessions,
    OpenSession(PathBuf),
    /// Move a session file to `trash/` inside the sessions folder.
    DeleteSession(PathBuf),
    /// Write the visible conversation as Markdown.
    Export(Option<String>),
    /// Put text on the system clipboard.
    Copy(String),
    RefreshTokens,
    RefreshGit,
    ValidateKey {
        provider: String,
        key: String,
        base_url: String,
    },
    SaveAuth {
        provider: String,
        key: String,
    },
    SaveBaseUrl {
        provider: String,
        base_url: String,
    },
    FetchModels {
        provider: String,
    },
    SetModel {
        provider: String,
        id: String,
    },
    Disconnect {
        provider: String,
    },
    OpenUrl(String),
    /// List project files for `@` mentions.
    ScanFiles,
    /// Run a `!command` locally, without the model.
    RunLocal {
        command: String,
    },
    /// Write `<ARQUS_HOME>/state.json` after a remembered choice changed.
    SaveState,
    /// Tell the engine which approval mode the next turn uses.
    SetMode(Mode),
    /// Turn automatic compaction on or off for the next turn.
    SetCompaction(bool),
    /// Ring the terminal bell once.
    Bell,
    /// Turn mouse capture on or off.
    SetMouse(bool),
}
