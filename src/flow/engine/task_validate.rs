use std::collections::BTreeMap;

use crate::ai::{default_context_window, Llm, ModelConfig};
use crate::flow::config::types::{ResolvedFlowConfig, WorkerConfig};
use crate::flow::schema::types::{Assignee, RunState, TaskContract, TaskState};
use crate::flow::schema::{validate_shape, validate_task_id};

pub fn is_revised_contract_valid(
    revised: &TaskContract,
    state: &RunState,
    config: &ResolvedFlowConfig,
) -> bool {
    if !validate_task_id(&revised.id)
        || !validate_shape(revised).is_empty()
        || revised.allowed_writes.is_empty()
    {
        return false;
    }
    if revised
        .depends_on
        .iter()
        .any(|d| !state.tasks.contains_key(d) || d == &revised.id)
    {
        return false;
    }
    if revised.assignee == Assignee::Worker {
        let def = &config.routing.default_worker;
        let worker = match config
            .workers
            .iter()
            .find(|w| &w.name == def)
            .or_else(|| config.workers.first())
        {
            Some(w) => w,
            None => return false,
        };
        if revised.context.len() > worker.limits.max_context_files as usize
            || revised.allowed_writes.len() > worker.limits.max_write_files as usize
            || revised.est_changed_lines > worker.limits.max_est_changed_lines
        {
            return false;
        }
    }
    true
}

pub fn are_subtasks_valid(
    subtasks: &[TaskContract],
    task_state: &TaskState,
    config: &ResolvedFlowConfig,
) -> bool {
    if subtasks.is_empty() || task_state.split_depth >= config.limits.max_split_depth {
        return false;
    }
    subtasks
        .iter()
        .all(|s| validate_shape(s).is_empty() && !s.allowed_writes.is_empty())
}

pub fn build_worker_llm(worker: &WorkerConfig, keys: &BTreeMap<String, String>) -> Llm {
    let api_key = keys
        .get(&worker.name)
        .cloned()
        .unwrap_or_else(|| "local".to_string());
    Llm::from_config(ModelConfig {
        provider: worker.provider.clone(),
        model: worker.model.clone(),
        base_url: worker.base_url.clone(),
        api_key,
        context_window: default_context_window(&worker.provider),
        extra_body: worker.extra_body.clone(),
    })
}

#[cfg(test)]
#[path = "task_validate_tests.rs"]
mod tests;
