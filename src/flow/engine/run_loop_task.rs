use serde_json::json;

use crate::ai::Llm;
use crate::flow::engine::events::emit_event;
use crate::flow::engine::router::{DefaultRouter, Router};
use crate::flow::engine::task_accept::{accept_task, run_takeover_flow, TaskContext};
use crate::flow::engine::task_validate::{
    are_subtasks_valid, build_worker_llm, is_revised_contract_valid,
};
use crate::flow::errors::FlowError;
use crate::flow::exec::worker::{run_worker_rounds, WorkerContext, WorkerRoundResult};
use crate::flow::model::ledger::Ledger;
use crate::flow::planning::redesign::redesign;
use crate::flow::schema::types::{Assignee, RedesignDecision, RunState, TaskStatus};

pub async fn execute_task(
    task_id: &str,
    run_state: &mut RunState,
    orch_llm: &Llm,
    router: &DefaultRouter,
    ctx: &TaskContext<'_>,
    ledger: &mut Ledger,
) -> Result<(), FlowError> {
    run_state.transition_task(task_id, TaskStatus::Running)?;
    run_state.save(ctx.state_path)?;
    emit_event(
        ctx.events_path,
        "task_status",
        Some(task_id),
        json!({ "status": "running" }),
    );

    let is_orch = run_state
        .tasks
        .get(task_id)
        .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?
        .contract
        .assignee
        == Assignee::Orchestrator;
    if is_orch {
        return run_takeover_flow(task_id, run_state, orch_llm, ctx, ledger).await;
    }

    loop {
        let (worker_config, contract) = {
            let task = run_state
                .tasks
                .get(task_id)
                .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
            (router.select(&task.contract).clone(), task.contract.clone())
        };
        let worker_llm = build_worker_llm(&worker_config, &ctx.config.worker_keys);
        let mut task_state = run_state
            .tasks
            .remove(task_id)
            .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
        let w_ctx = WorkerContext {
            worktree: ctx.worktree,
            run_state,
            limits: &ctx.config.limits,
            sandbox: &ctx.config.sandbox,
            run_id: &run_state.run_id,
        };
        let res = run_worker_rounds(
            &worker_llm,
            &worker_config,
            &contract,
            &mut task_state,
            &w_ctx,
            Some(ledger),
        )
        .await;
        run_state.tasks.insert(task_id.to_string(), task_state);

        match res {
            WorkerRoundResult::Accepted { changed_files } => {
                accept_task(
                    task_id,
                    run_state,
                    changed_files,
                    Assignee::Worker,
                    &worker_config.model,
                    ctx,
                )?;
                return Ok(());
            }
            WorkerRoundResult::Failed(record) => {
                let redesigns = {
                    let task_mut = run_state
                        .tasks
                        .get_mut(task_id)
                        .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
                    task_mut.failures.push(record);
                    if task_mut.redesigns >= ctx.config.limits.max_redesigns {
                        return run_takeover_flow(task_id, run_state, orch_llm, ctx, ledger).await;
                    }
                    task_mut.status = TaskStatus::Redesigning;
                    task_mut.redesigns += 1;
                    task_mut.redesigns
                };
                run_state.save(ctx.state_path)?;
                emit_event(
                    ctx.events_path,
                    "task_status",
                    Some(task_id),
                    json!({ "status": "redesigning", "redesigns": redesigns }),
                );

                let task_ref = run_state
                    .tasks
                    .get(task_id)
                    .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
                let decision = redesign(
                    orch_llm,
                    &ctx.config.orchestrator,
                    &task_ref.contract,
                    &task_ref.failures,
                    ctx.capability_sheet,
                    Some(ledger),
                    &run_state.run_id,
                )
                .await;
                emit_event(
                    ctx.events_path,
                    "redesign_decision",
                    Some(task_id),
                    json!({ "decision": decision }),
                );

                if let RedesignDecision::Revise { revised, .. } = &decision {
                    if is_revised_contract_valid(revised, run_state, ctx.config) {
                        let t = run_state.tasks.get_mut(task_id).ok_or_else(|| {
                            FlowError::Config(format!("task '{task_id}' not found"))
                        })?;
                        t.contract = *revised.clone();
                        t.worker_attempts = 0;
                        t.status = TaskStatus::Running;
                        run_state.save(ctx.state_path)?;
                        emit_event(
                            ctx.events_path,
                            "task_status",
                            Some(task_id),
                            json!({ "status": "running" }),
                        );
                        continue;
                    }
                } else if let RedesignDecision::Split { split, .. } = &decision {
                    let t = run_state
                        .tasks
                        .get(task_id)
                        .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
                    if are_subtasks_valid(split, t, ctx.config) {
                        run_state.replace_with_subtasks(task_id, split.clone())?;
                        run_state.save(ctx.state_path)?;
                        emit_event(
                            ctx.events_path,
                            "task_status",
                            Some(task_id),
                            json!({ "status": "superseded" }),
                        );
                        return Ok(());
                    }
                }
                return run_takeover_flow(task_id, run_state, orch_llm, ctx, ledger).await;
            }
        }
    }
}

#[cfg(test)]
#[path = "run_loop_task_tests.rs"]
mod tests;
