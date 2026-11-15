use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::flow::errors::FlowError;
use crate::flow::model::ledger::{Ledger, Role};
use crate::flow::schema::types::{Assignee, Category, RunState, TaskStatus};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TaskStatRecord {
    pub model: String,
    pub category: Category,
    pub first_attempt_pass: bool,
    pub attempts: u32,
    pub redesigns: u32,
    pub takeover: bool,
    pub tokens: u64,
}

pub fn build_task_stats(
    state: &RunState,
    ledger: &Ledger,
    default_worker_model: &str,
    orch_model: &str,
) -> Vec<TaskStatRecord> {
    let mut stats = Vec::new();
    let records = ledger.records();

    for (task_id, task_state) in &state.tasks {
        if task_state.status == TaskStatus::Superseded {
            continue;
        }

        let mut in_tok = 0u64;
        let mut out_tok = 0u64;
        let mut task_worker_model = None;
        let mut task_other_model = None;

        for rec in records {
            if rec.task_id.as_deref() == Some(task_id.as_str()) {
                in_tok = in_tok.saturating_add(rec.input);
                out_tok = out_tok.saturating_add(rec.output);
                if rec.role == Role::Worker && task_worker_model.is_none() {
                    task_worker_model = Some(rec.model.clone());
                } else if task_other_model.is_none() {
                    task_other_model = Some(rec.model.clone());
                }
            }
        }

        let model = if let Some(wm) = task_worker_model {
            wm
        } else if let Some(om) = task_other_model {
            om
        } else if task_state.contract.assignee == Assignee::Worker {
            default_worker_model.to_string()
        } else {
            orch_model.to_string()
        };

        let takeover = task_state.takeover_attempts > 0
            || task_state.executor == Assignee::Orchestrator
            || task_state.contract.assignee == Assignee::Orchestrator;

        let attempts = task_state.worker_attempts + task_state.takeover_attempts;

        let first_attempt_pass = if task_state.status != TaskStatus::Done {
            false
        } else if task_state.contract.assignee == Assignee::Worker {
            task_state.worker_attempts == 1
                && task_state.redesigns == 0
                && task_state.takeover_attempts == 0
                && task_state.executor == Assignee::Worker
        } else {
            task_state.takeover_attempts == 1 && task_state.redesigns == 0
        };

        stats.push(TaskStatRecord {
            model,
            category: task_state.contract.category,
            first_attempt_pass,
            attempts,
            redesigns: task_state.redesigns,
            takeover,
            tokens: in_tok.saturating_add(out_tok),
        });
    }

    stats
}

pub fn append_run_stats(path: &Path, records: &[TaskStatRecord]) -> Result<(), FlowError> {
    if records.is_empty() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| FlowError::Config(format!("failed to create stats dir: {e}")))?;
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| {
            FlowError::Config(format!("failed to open stats file {}: {e}", path.display()))
        })?;

    for rec in records {
        let mut line = serde_json::to_string(rec)
            .map_err(|e| FlowError::Config(format!("failed to serialize task stat record: {e}")))?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .map_err(|e| FlowError::Config(format!("failed to write stats record: {e}")))?;
    }

    file.flush()
        .map_err(|e| FlowError::Config(format!("failed to flush stats file: {e}")))?;

    Ok(())
}

pub fn record_run_stats(
    state: &RunState,
    ledger: &Ledger,
    default_worker_model: &str,
    orch_model: &str,
) -> Result<(), FlowError> {
    let records = build_task_stats(state, ledger, default_worker_model, orch_model);
    let stats_path = crate::home::arqus_home().join("flow").join("stats.jsonl");
    append_run_stats(&stats_path, &records)
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "stats_build_tests.rs"]
mod build_tests;
