use serde_json::json;

use crate::ai::{default_context_window, Llm, ModelConfig};
use crate::flow::cli::RunArgs;
use crate::flow::config::load_flow_config;
use crate::flow::config::types::FlowCliOverrides;
use crate::flow::engine::events::emit_event;
use crate::flow::engine::router::DefaultRouter;
use crate::flow::engine::run_loop_task::execute_task;
use crate::flow::engine::scheduler::Scheduler;
use crate::flow::exec::verify::run_verify_commands;
use crate::flow::model::ledger::Ledger;
use crate::flow::planning::capability::render_capability_sheet;
use crate::flow::planning::planner::Planner;
use crate::flow::repo::git::{
    check_clean_repo, create_branch_and_worktree, ensure_exclude_entry, get_head_commit,
    list_tracked_files,
};
use crate::flow::repo::map::build_repo_map;
use crate::flow::schema::types::{RunState, RunStatus};

pub async fn run_flow(run_args: &RunArgs) -> anyhow::Result<()> {
    let repo_path = std::fs::canonicalize(&run_args.repo)
        .map_err(|e| anyhow::anyhow!("failed to resolve repo: {e}"))?;
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let overrides = FlowCliOverrides {
        orchestrator_model: run_args.orchestrator_model.clone(),
        worker: run_args.worker.clone(),
        max_cost: run_args.max_cost,
        max_cloud_tokens: run_args.max_cloud_tokens,
    };
    let config =
        load_flow_config(&home, &repo_path, &overrides).map_err(|e| anyhow::anyhow!("{e}"))?;

    check_clean_repo(&repo_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let base_commit = get_head_commit(&repo_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let base_files = list_tracked_files(&repo_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let repo_map = build_repo_map(&repo_path, 32_000).map_err(|e| anyhow::anyhow!("{e}"))?;

    let default_name = &config.routing.default_worker;
    let default_worker = config
        .workers
        .iter()
        .find(|w| &w.name == default_name)
        .or_else(|| config.workers.first())
        .ok_or_else(|| anyhow::anyhow!("no workers"))?;
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
    let run_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let mut plan_ledger = Ledger::new(None);

    let plan = {
        let mut p = Planner {
            llm: &orch_llm,
            config: &config,
            repo_path: &repo_path,
            base_files: &base_files,
            repo_map: &repo_map,
            capability_sheet: &capability_sheet,
            ledger: &mut plan_ledger,
            run_id: &run_id,
        };
        p.plan(&run_args.objective)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?
    };

    ensure_exclude_entry(&repo_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let (branch, worktree) = create_branch_and_worktree(&repo_path, &run_id, Some(&base_commit))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let run_dir = worktree
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing worktree parent"))?;
    let (state_path, usage_path, events_path) = (
        run_dir.join("state.json"),
        run_dir.join("usage.jsonl"),
        run_dir.join("events.jsonl"),
    );

    let mut ledger = Ledger::new(Some(usage_path));
    for rec in plan_ledger.records() {
        let _ = ledger.record(rec.clone());
    }

    let mut run_state = RunState::from_plan(
        &plan,
        run_id.clone(),
        run_args.objective.clone(),
        repo_path.display().to_string(),
        base_commit,
        branch,
    );
    run_state
        .save(&state_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    emit_event(
        &events_path,
        "run_started",
        None,
        json!({ "run_id": run_id, "objective": run_args.objective }),
    );
    emit_event(
        &events_path,
        "plan_created",
        None,
        json!({ "summary": plan.summary, "task_count": plan.tasks.len() }),
    );

    let router = DefaultRouter::from_config(&config).map_err(|e| anyhow::anyhow!("{e}"))?;
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
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    let v_res = run_verify_commands(
        &worktree,
        &run_state.final_verify,
        &config.sandbox.allowed_commands,
        &config.sandbox.env_passthrough,
        config.limits.verify_timeout_secs,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let success = Scheduler::all_done(&run_state) && v_res.passed;
    run_state.status = if success {
        RunStatus::Finished
    } else {
        RunStatus::Failed
    };
    run_state
        .save(&state_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    emit_event(
        &events_path,
        "run_finished",
        None,
        json!({ "status": if success { "finished" } else { "failed" } }),
    );

    if success {
        Ok(())
    } else {
        anyhow::bail!("execution failed: task failed or final verify failed");
    }
}

#[cfg(test)]
#[path = "run_loop_tests.rs"]
mod tests;
