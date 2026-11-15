use std::path::Path;

use crate::flow::errors::FlowError;
use crate::flow::model::ledger::{Ledger, LedgerTotals};
use crate::flow::schema::types::{Assignee, RunState, RunStatus, TaskStatus};

#[derive(Debug, Clone, PartialEq)]
pub struct TaskReportRow {
    pub id: String,
    pub title: String,
    pub executor: Assignee,
    pub attempts: u32,
    pub redesigns: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskCounts {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowReport {
    pub run_id: String,
    pub objective: String,
    pub status: RunStatus,
    pub tasks: Vec<TaskReportRow>,
    pub totals: LedgerTotals,
    pub task_counts: TaskCounts,
}

pub fn generate_report(state: &RunState, ledger: &Ledger) -> FlowReport {
    let mut tasks = Vec::new();
    let mut task_counts = TaskCounts {
        total: state.tasks.len(),
        done: 0,
        failed: 0,
        skipped: 0,
    };

    let records = ledger.records();

    for (task_id, task_state) in &state.tasks {
        match task_state.status {
            TaskStatus::Done => task_counts.done += 1,
            TaskStatus::Failed => task_counts.failed += 1,
            TaskStatus::Skipped => task_counts.skipped += 1,
            _ => {}
        }

        let mut in_tok = 0u64;
        let mut out_tok = 0u64;
        for rec in records {
            if rec.task_id.as_deref() == Some(task_id.as_str()) {
                in_tok = in_tok.saturating_add(rec.input);
                out_tok = out_tok.saturating_add(rec.output);
            }
        }

        tasks.push(TaskReportRow {
            id: task_id.clone(),
            title: task_state.contract.title.clone(),
            executor: task_state.executor,
            attempts: task_state.worker_attempts + task_state.takeover_attempts,
            redesigns: task_state.redesigns,
            input_tokens: in_tok,
            output_tokens: out_tok,
            status: task_state.status,
        });
    }

    FlowReport {
        run_id: state.run_id.clone(),
        objective: state.objective.clone(),
        status: state.status,
        tasks,
        totals: ledger.totals(),
        task_counts,
    }
}

pub fn render_report(report: &FlowReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Run: {}\nObjective: {}\nStatus: {}\n\n",
        report.run_id,
        report.objective,
        format!("{:?}", report.status).to_lowercase()
    ));
    out.push_str("Tasks:\n");
    for t in &report.tasks {
        out.push_str(&format!(
            "  [{}] {} | {:?} | attempts: {} | redesigns: {} | tokens: {} in / {} out | {:?}\n",
            t.id,
            t.title,
            t.executor,
            t.attempts,
            t.redesigns,
            t.input_tokens,
            t.output_tokens,
            t.status
        ));
    }
    out.push_str("\nTotals:\n");
    let cloud_cost = report
        .totals
        .cloud
        .cost_usd
        .map(|c| format!("${:.4}", c))
        .unwrap_or_else(|| "$0.0000".to_string());
    out.push_str(&format!(
        "  Cloud: {} in / {} out / {} cache read ({})\n",
        report.totals.cloud.input,
        report.totals.cloud.output,
        report.totals.cloud.cache_read,
        cloud_cost
    ));
    out.push_str(&format!(
        "  Local: {} in / {} out\n",
        report.totals.local.input, report.totals.local.output
    ));
    out.push_str(&format!(
        "  Done: {}/{} tasks ({} failed, {} skipped)\n",
        report.task_counts.done,
        report.task_counts.total,
        report.task_counts.failed,
        report.task_counts.skipped
    ));
    out
}

pub fn load_report_from_dir(repo_path: &Path, run_id: &str) -> Result<FlowReport, FlowError> {
    let run_dir = repo_path.join(".arqus").join("flow").join(run_id);
    let state_file = run_dir.join("state.json");
    if !state_file.exists() {
        return Err(FlowError::Config(format!(
            "run '{run_id}' not found at {}",
            state_file.display()
        )));
    }
    let state = RunState::load(&state_file)?;
    let usage_file = run_dir.join("usage.jsonl");
    let ledger = Ledger::load(&usage_file).map_err(|e| FlowError::Config(e.to_string()))?;
    Ok(generate_report(&state, &ledger))
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
