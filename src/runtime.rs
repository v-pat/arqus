use anyhow::Result;
use tokio::sync::mpsc::Sender;

use crate::ai::Llm;
use crate::context::build_context;
use crate::session::Session;
use crate::types::EntryKind;

use super::{run_prompt, AgentConfig, AgentEvent};

/// A ready-to-run agent: one model connection plus its settings.
///
/// The TUI holds this type instead of importing `ai` directly.
#[derive(Debug, Clone)]
pub struct Runtime {
    pub llm: Llm,
    pub cfg: AgentConfig,
    pub model_name: String,
}

impl Runtime {
    pub async fn run(
        &self,
        session: &mut Session,
        text: String,
        events: Sender<AgentEvent>,
        steer_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
        approval_rx: tokio::sync::mpsc::Receiver<crate::safety::Decision>,
    ) -> Result<()> {
        run_prompt(
            session,
            &self.llm,
            &self.cfg,
            text,
            events,
            steer_rx,
            approval_rx,
        )
        .await
    }

    pub async fn compact(&self, session: &mut Session, focus: Option<String>) -> Result<bool> {
        crate::compaction::compact(session, &self.llm, &self.cfg.compaction, focus.as_deref()).await
    }

    /// Switch provider and model, and record the change in the session.
    /// Later requests read neutral messages, so mixed-provider history works.
    pub fn switch_model(&mut self, session: &mut Session, spec: &str) -> Result<()> {
        let cfg = crate::ai::ModelConfig::from_model_spec(spec)?;
        self.apply_model(session, cfg)
    }

    /// Apply a fully resolved config and record the change in the session.
    pub fn apply_model(
        &mut self,
        session: &mut Session,
        cfg: crate::ai::ModelConfig,
    ) -> Result<()> {
        session.append(EntryKind::ModelChange {
            provider: cfg.provider.clone(),
            model: cfg.model.clone(),
        })?;
        self.model_name = if cfg.model.is_empty() {
            "(no model)".to_string()
        } else {
            cfg.model.clone()
        };
        self.cfg.context_window = cfg.context_window;
        self.llm = Llm::from_config(cfg);
        Ok(())
    }

    /// Size of the context the next request would send, in tokens.
    pub fn context_tokens(&self, session: &Session) -> u64 {
        let messages = build_context(&session.entries, session.leaf_id.as_deref());
        crate::compaction::context_tokens(&messages)
    }

    /// Money left on the OpenRouter key, as one line.
    pub async fn budget(&self) -> Result<String> {
        match &self.llm {
            Llm::OpenRouter(cfg) => crate::ai::openrouter::budget(cfg).await,
            _ => Ok("budget: only available for the openrouter provider".to_string()),
        }
    }
}
