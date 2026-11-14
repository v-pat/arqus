//! The approval step around one tool call.
//!
//! Pure checks (path guard, risky patterns) live in `safety`. This module
//! connects them to the agent loop and the approval channel.

use serde_json::Value;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::safety::{self, Approver, Decision, PathOp};
use crate::tools::ToolOutput;

use super::{AgentConfig, AgentEvent};

pub const RISKY_MARKER: &str = "RISKY";
pub const DENIED: &str = "The user denied this tool call.";
pub const APPROVAL_REQUIRED: &str = "Approval required. Run arqus with --yolo to allow this.";
pub const PLAN_REFUSAL: &str = "Plan mode: changes are disabled. Describe the plan instead.";
pub const PLAN_NOTE: &str =
    "Plan mode: changes are disabled. Describe the plan instead. Only read is permitted; do not call write, edit, or bash.";

/// Check one call. `Err` is the tool result to send back to the model.
///
/// Order: path guard, risky command, approval. The path guard runs in every
/// mode, so `--yolo` still cannot write outside the project.
pub async fn approve(
    name: &str,
    args: &Value,
    cfg: &AgentConfig,
    events: &Sender<AgentEvent>,
    approval_rx: &mut Receiver<Decision>,
) -> Result<(), ToolOutput> {
    // Plan mode: reads still work, every change is refused with one clear line.
    if cfg.plan && name != "read" {
        let is_mcp_read = if let Some((_, tool_name)) = crate::mcp::parse_tool_name(name) {
            let prefix = tool_name.split('_').next().unwrap_or("");
            matches!(
                prefix,
                "get" | "list" | "find" | "search" | "read" | "inspect" | "check"
            )
        } else {
            false
        };
        if !is_mcp_read {
            return Err(ToolOutput::error(PLAN_REFUSAL));
        }
    }

    if let Some(path) = args.get("path").and_then(Value::as_str) {
        let op = match name {
            "read" => Some(PathOp::Read),
            "write" | "edit" => Some(PathOp::Write),
            _ => None,
        };
        if let Some(op) = op {
            cfg.guard.check(path, op).map_err(ToolOutput::error)?;
        }
    }

    let risky = if name == "bash" {
        let command = args.get("command").and_then(Value::as_str).unwrap_or("");
        safety::is_risky(command, &cfg.cwd, cfg.guard.user_home.as_deref())
    } else {
        None
    };

    if risky.is_some() && cfg.approver == Approver::Auto && !cfg.allow_risky {
        let reason = risky.clone().unwrap_or_default();
        return Err(ToolOutput::error(format!(
            "refused risky command: {reason} (start arqus with --allow-risky to allow it)"
        )));
    }

    // Reads never ask.
    if name == "read" {
        return Ok(());
    }

    match cfg.approver {
        Approver::Auto => Ok(()),
        Approver::DenyAll => Err(ToolOutput::error(APPROVAL_REQUIRED)),
        Approver::Interactive => {
            if cfg.approvals.is_always(name) && (risky.is_none() || cfg.allow_risky) {
                return Ok(());
            }
            let preview = preview(name, args, risky.as_deref());
            let request = AgentEvent::ApprovalRequest {
                tool: name.to_string(),
                preview,
            };
            if events.send(request).await.is_err() {
                return Err(ToolOutput::error(DENIED));
            }
            match approval_rx.recv().await {
                Some(Decision::Allow) => Ok(()),
                Some(Decision::AllowAlways) => {
                    cfg.approvals.remember(name);
                    Ok(())
                }
                Some(Decision::Deny) | None => Err(ToolOutput::error(DENIED)),
            }
        }
    }
}

/// Text the approval modal shows. A risky command gets a marker as its first
/// line; the TUI draws that line in red.
pub fn preview(tool: &str, args: &Value, risky: Option<&str>) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(reason) = risky {
        lines.push(RISKY_MARKER.to_string());
        lines.push(format!("risky pattern: {reason}"));
        lines.push(String::new());
    }

    match tool {
        "bash" => lines.push(
            args.get("command")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
        "write" => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            let content = args.get("content").and_then(Value::as_str).unwrap_or("");
            lines.push(format!("path: {path}"));
            lines.push(String::new());
            push_head(&mut lines, content, 20);
        }
        "edit" => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            lines.push(format!("path: {path}"));
            lines.push(String::new());
            lines.push("--- old".to_string());
            lines.push(
                args.get("old_text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            );
            lines.push("+++ new".to_string());
            lines.push(
                args.get("new_text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            );
        }
        other if other.starts_with(crate::mcp::MCP_PREFIX) => {
            if let Some((server, tool_name)) = crate::mcp::parse_tool_name(other) {
                lines.push(format!("mcp server: {server}"));
                lines.push(format!("mcp tool:   {tool_name}"));
                lines.push(String::new());
                lines.push("arguments:".to_string());
                if let Ok(pretty) = serde_json::to_string_pretty(args) {
                    push_head(&mut lines, &pretty, 20);
                } else {
                    lines.push(args.to_string());
                }
            } else {
                lines.push(format!("tool: {other}"));
            }
        }
        other => lines.push(format!("tool: {other}")),
    }
    lines.join("\n")
}

fn push_head(lines: &mut Vec<String>, text: &str, max: usize) {
    for line in text.lines().take(max) {
        lines.push(line.to_string());
    }
    let total = text.lines().count();
    if total > max {
        lines.push(format!("... {} more lines", total - max));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::Limits;
    use crate::safety::{Approvals, Approver, Guard};
    use serde_json::json;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn allow_always_does_not_skip_risky_commands() {
        let (events_tx, mut events_rx) = mpsc::channel(10);
        let (approval_tx, mut approval_rx) = mpsc::channel(1);

        let approvals = Approvals::default();
        approvals.remember("bash");

        let cfg = AgentConfig {
            cwd: std::env::temp_dir(),
            system_prompt: String::new(),
            context_window: 128000,
            compaction: Default::default(),
            limits: Limits::default(),
            approver: Approver::Interactive,
            allow_risky: false,
            plan: false,
            approvals,
            guard: Guard::default(),
            mcp: None,
        };

        // 1. Safe command is allowed automatically
        let safe_args = json!({"command": "echo hello"});
        let res = approve("bash", &safe_args, &cfg, &events_tx, &mut approval_rx).await;
        assert!(res.is_ok());

        // 2. Risky command must still ask for approval despite allow_always
        let risky_args = json!({"command": "sudo rm -rf /"});
        tokio::spawn(async move {
            approval_tx.send(Decision::Deny).await.unwrap();
        });
        let res = approve("bash", &risky_args, &cfg, &events_tx, &mut approval_rx).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.text, DENIED);

        let ev = events_rx.recv().await.unwrap();
        match ev {
            AgentEvent::ApprovalRequest { tool, preview } => {
                assert_eq!(tool, "bash");
                assert!(preview.contains(RISKY_MARKER));
            }
            other => panic!("expected ApprovalRequest, got {other:?}"),
        }
    }
}
