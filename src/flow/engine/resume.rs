use std::path::Path;
use std::time::Instant;

use serde_json::json;

use crate::ai::{default_context_window, Llm, ModelConfig};
use crate::flow::cli::ResumeArgs;
use crate::flow::config::load_flow_config;
use crate::flow::config::types::FlowCliOverrides;
use crate::flow::engine::events::emit_event;
use crate::flow::engine::router::DefaultRouter;
use crate::flow::engine::run_loop_task::execute_task;
use crate::flow::engine::scheduler::Scheduler;
use crate::flow::errors::FlowError;
use crate::flow::exec::verify::run_verify_commands;
use crate::flow::model::ledger::Ledger;
use crate::flow::planning::capability::render_capability_sheet;
use crate::flow::repo::git::{get_head_commit, reset_hard_clean};
use crate::flow::schema::types::{RunState, RunStatus, TaskStatus};

pub fn find_last_accepted_commit(state: &RunState, events_path: Option<&Path>) -> String {
    if let Some(path) = events_path {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                let mut last_commit = None;
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        if val.get("type").and_then(|v| v.as_str()) == Some("task_accepted") {
                            if let Some(c) = val
                                .get("data")
                                .and_then(|d| d.get("commit"))
                                .and_then(|c| c.as_str())
                            {
                                last_commit = Some(c.to_string());
                            }
                        }
                    }
                }
                if let Some(c) = last_commit {
                    return c;
                }
            }
        }
    }

    let mut done_commit = None;
    for task in state.tasks.values() {
        if task.status == TaskStatus::Done {
            if let Some(ref c) = task.commit {
                done_commit = Some(c.clone());
            }
        }
    }

    done_commit.unwrap_or_else(|| state.base_commit.clone())
}

pub fn reset_tasks_for_resume(state: &mut RunState) {
    for task in state.tasks.values_mut() {
        if task.status == TaskStatus::Running || task.status == TaskStatus::Redesigning {
            task.status = TaskStatus::Ready;
            task.worker_attempts = 0;
            task.takeover_attempts = 0;
        }
    }
    if state.status == RunStatus::BudgetStopped || state.status == RunStatus::Running {
        state.status = RunStatus::Running;
    }
}

pub async fn run_resume(args: &ResumeArgs) -> anyhow::Result<()> {
    let start_time = Instant::now();
    let repo_path = std::fs::canonicalize(&args.repo)
        .map_err(|e| anyhow::anyhow!("failed to resolve repo path '{}': {e}", args.repo))?;

    let run_dir = repo_path.join(".arqus").join("flow").join(&args.run_id);
    let state_path = run_dir.join("state.json");
    if !state_path.exists() {
        return Err(anyhow::Error::new(FlowError::Config(format!(
            "flow run '{}' not found at {}",
            args.run_id,
            state_path.display()
        ))));
    }

    let mut run_state = RunState::load(&state_path).map_err(anyhow::Error::new)?;
    let worktree = run_dir.join("worktree");
    let events_path = run_dir.join("events.jsonl");
    let usage_path = run_dir.join("usage.jsonl");

    let expected_commit = find_last_accepted_commit(&run_state, Some(&events_path));
    let head = get_head_commit(&worktree).map_err(anyhow::Error::new)?;
    if head != expected_commit {
        return Err(anyhow::Error::new(FlowError::Git(format!(
            "worktree HEAD ({head}) does not match last accepted commit ({expected_commit})"
        ))));
    }

    reset_hard_clean(&worktree).map_err(anyhow::Error::new)?;
    reset_tasks_for_resume(&mut run_state);
    run_state.save(&state_path).map_err(anyhow::Error::new)?;

    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let overrides = FlowCliOverrides::default();
    let config = load_flow_config(&home, &repo_path, &overrides).map_err(anyhow::Error::new)?;

    let default_name = &config.routing.default_worker;
    let default_worker = config
        .workers
        .iter()
        .find(|w| &w.name == default_name)
        .or_else(|| config.workers.first())
        .ok_or_else(|| FlowError::Config("no workers configured".to_string()))?;
    let p_stat = home.join(".arqus/flow/profile_stats.json");
    let capability_sheet =
        render_capability_sheet(default_worker, p_stat.exists().then_some(p_stat.as_path()));

    let orch_cfg = ModelConfig {
        provider: config.orchestrator.provider.clone(),
        model: config.orchestrator.model.clone(),
        base_url: config.orchestrator.base_url.clone(),
        api_key: config.orchestrator_key.clone(),
        context_window: default_context_window(&config.orchestrator.provider),
        extra_body: config.orchestrator.extra_body.clone(),
    };
    let orch_llm = Llm::from_config(orch_cfg);
    let mut ledger = Ledger::load(&usage_path).map_err(anyhow::Error::new)?;

    let router = DefaultRouter::from_config(&config).map_err(anyhow::Error::new)?;
    let task_ctx = crate::flow::engine::task_accept::TaskContext {
        config: &config,
        worktree: &worktree,
        capability_sheet: &capability_sheet,
        state_path: &state_path,
        events_path: &events_path,
    };

    while let Some(task_id) = Scheduler::next_ready(&run_state) {
        execute_task(
            &task_id,
            &mut run_state,
            &orch_llm,
            &router,
            &task_ctx,
            &mut ledger,
        )
        .await
        .map_err(anyhow::Error::new)?;
    }

    let v_res = run_verify_commands(
        &worktree,
        &run_state.final_verify,
        &config.sandbox.allowed_commands,
        &config.sandbox.env_passthrough,
        config.limits.verify_timeout_secs,
    )
    .await
    .map_err(anyhow::Error::new)?;

    let success = Scheduler::all_done(&run_state) && v_res.passed;
    run_state.status = if success {
        RunStatus::Finished
    } else {
        RunStatus::Failed
    };
    run_state.save(&state_path).map_err(anyhow::Error::new)?;
    emit_event(
        &events_path,
        "run_finished",
        None,
        json!({ "status": if success { "finished" } else { "failed" } }),
    );

    let default_worker_model = config
        .workers
        .iter()
        .find(|w| w.name == config.routing.default_worker)
        .or_else(|| config.workers.first())
        .map(|w| w.model.as_str())
        .unwrap_or("worker");
    let _ = crate::flow::engine::stats::record_run_stats(
        &run_state,
        &ledger,
        default_worker_model,
        &config.orchestrator.model,
    );

    let totals = ledger.totals();
    let elapsed = start_time.elapsed().as_secs();
    let mut done_count = 0;
    let mut failed_count = 0;
    for t in run_state.tasks.values() {
        if t.status == TaskStatus::Done {
            done_count += 1;
        } else if t.status == TaskStatus::Failed {
            failed_count += 1;
        }
    }
    let cost_str = totals
        .cloud
        .cost_usd
        .map(|c| format!("${:.4}", c))
        .unwrap_or_else(|| "$0.0000".to_string());
    eprintln!(
        "[arqus flow] tasks={} done={} failed={} cloud_in={} cloud_out={} local_in={} local_out={} cost={} time={}s",
        run_state.tasks.len(),
        done_count,
        failed_count,
        totals.cloud.input,
        totals.cloud.output,
        totals.local.input,
        totals.local.output,
        cost_str,
        elapsed
    );

    if success {
        Ok(())
    } else {
        Err(anyhow::Error::new(FlowError::VerificationFailed(
            "execution failed: task failed or final verify failed".to_string(),
        )))
    }
}

#[cfg(test)]
#[path = "resume_tests.rs"]
mod tests;
