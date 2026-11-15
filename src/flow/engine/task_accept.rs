use std::path::Path;

use serde_json::json;

use crate::ai::Llm;
use crate::flow::config::types::ResolvedFlowConfig;
use crate::flow::engine::events::emit_event;
use crate::flow::errors::FlowError;
use crate::flow::exec::takeover::{run_takeover_rounds, TakeoverContext, TakeoverRoundResult};
use crate::flow::model::ledger::Ledger;
use crate::flow::repo::git::commit_changes;
use crate::flow::schema::types::{Assignee, RunState, TaskStatus};

pub struct TaskContext<'a> {
    pub config: &'a ResolvedFlowConfig,
    pub worktree: &'a Path,
    pub capability_sheet: &'a str,
    pub state_path: &'a Path,
    pub events_path: &'a Path,
}

pub fn accept_task(
    task_id: &str,
    state: &mut RunState,
    changed: Vec<String>,
    exec: Assignee,
    model: &str,
    ctx: &TaskContext<'_>,
) -> Result<(), FlowError> {
    let (t_id, t_title) = {
        let t = state
            .tasks
            .get(task_id)
            .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
        (t.contract.id.clone(), t.contract.title.clone())
    };
    let commit = commit_changes(ctx.worktree, &t_id, &t_title, &format!("{exec:?}"), model)?;
    let t_mut = state
        .tasks
        .get_mut(task_id)
        .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
    t_mut.commit = Some(commit.clone());
    t_mut.changed_files = changed;
    state.transition_task(task_id, TaskStatus::Done)?;
    state.save(ctx.state_path)?;
    emit_event(
        ctx.events_path,
        "task_accepted",
        Some(task_id),
        json!({ "executor": exec, "model": model, "commit": commit }),
    );
    Ok(())
}

pub async fn run_takeover_flow(
    task_id: &str,
    state: &mut RunState,
    orch_llm: &Llm,
    ctx: &TaskContext<'_>,
    ledger: &mut Ledger,
) -> Result<(), FlowError> {
    {
        let t = state
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
        t.executor = Assignee::Orchestrator;
        t.status = TaskStatus::Running;
    }
    state.save(ctx.state_path)?;
    emit_event(
        ctx.events_path,
        "task_status",
        Some(task_id),
        json!({ "status": "running", "executor": "orchestrator" }),
    );

    let contract = state
        .tasks
        .get(task_id)
        .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?
        .contract
        .clone();
    let mut task_state = state
        .tasks
        .remove(task_id)
        .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;

    let t_ctx = TakeoverContext {
        worktree: ctx.worktree,
        run_state: state,
        limits: &ctx.config.limits,
        sandbox: &ctx.config.sandbox,
        run_id: &state.run_id,
    };
    let res = run_takeover_rounds(
        orch_llm,
        &ctx.config.orchestrator,
        &contract,
        &mut task_state,
        &t_ctx,
        Some(ledger),
    )
    .await;
    state.tasks.insert(task_id.to_string(), task_state);

    match res {
        TakeoverRoundResult::Accepted { changed_files } => {
            accept_task(
                task_id,
                state,
                changed_files,
                Assignee::Orchestrator,
                &ctx.config.orchestrator.model,
                ctx,
            )?;
        }
        TakeoverRoundResult::Failed(record) => {
            let failures = {
                let t = state
                    .tasks
                    .get_mut(task_id)
                    .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found")))?;
                t.failures.push(record);
                t.failures.clone()
            };
            state.transition_task(task_id, TaskStatus::Failed)?;
            state.save(ctx.state_path)?;
            emit_event(
                ctx.events_path,
                "task_failed",
                Some(task_id),
                json!({ "failures": failures }),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "task_accept_tests.rs"]
mod tests;
