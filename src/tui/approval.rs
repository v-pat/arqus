//! The approval modal state: one pending tool call and its answer.

use crate::safety::Decision;

use super::app::App;

/// A tool call waiting for a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    pub tool: String,
    pub preview: String,
}

impl App {
    /// Store a pending approval request from the agent.
    pub fn approval_request(&mut self, tool: String, preview: String) {
        self.status = format!("approval needed: {tool}");
        self.approval = Some(Approval { tool, preview });
    }

    /// Send one answer to the agent. False when no request is pending.
    ///
    /// `try_send` is enough: the agent is waiting on the other end and the
    /// channel has room for the four possible answers.
    pub fn answer_approval(&mut self, decision: Decision) -> bool {
        if self.approval.take().is_none() {
            return false;
        }
        if let Some(sender) = &self.approval_tx {
            let _ = sender.try_send(decision);
        }
        if self.running {
            self.status = "waiting on approval".to_string();
        }
        true
    }
}
