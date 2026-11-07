//! Everything that can happen (`Msg`) and everything the world must do
//! (`Effect`). `update()` is the only function that turns one into the other.

use std::path::PathBuf;

use crate::agent::AgentEvent;
use crate::session::Session;

use super::keys::Key;

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
    Tick,
    Agent(AgentEvent),
    Done(Session),
    Toast(String, Level),
    Sessions(Vec<(PathBuf, String)>),
    Opened { session: Session, status: String },
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
    RefreshTokens,
}
