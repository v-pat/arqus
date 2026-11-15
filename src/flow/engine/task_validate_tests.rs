use super::*;
use crate::flow::config::defaults::default_flow_settings;
use crate::flow::schema::types::{Category, TaskStatus};

fn sample_config() -> ResolvedFlowConfig {
    let defaults = default_flow_settings();
    ResolvedFlowConfig {
        orchestrator: defaults.orchestrator.unwrap(),
        workers: defaults.workers.unwrap(),
        routing: defaults.routing.unwrap(),
        limits: defaults.limits.unwrap(),
        sandbox: defaults.sandbox.unwrap(),
        orchestrator_key: "orch-key".to_string(),
        worker_keys: BTreeMap::new(),
    }
}

fn sample_task(id: &str, assignee: Assignee) -> TaskContract {
    TaskContract {
        id: id.to_string(),
        title: format!("Task {id}"),
        category: Category::SingleFunction,
        assignee,
        objective: "test objective".to_string(),
        context: vec![],
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 10,
    }
}

fn sample_run_state(task: TaskContract) -> RunState {
    let mut tasks = BTreeMap::new();
    tasks.insert(
        task.id.clone(),
        TaskState {
            contract: task.clone(),
            status: TaskStatus::Running,
            executor: task.assignee,
            worker_attempts: 0,
            redesigns: 0,
            takeover_attempts: 0,
            split_depth: 0,
            failures: vec![],
            commit: None,
            changed_files: vec![],
        },
    );
    RunState {
        run_id: "test-run".to_string(),
        objective: "obj".to_string(),
        repo: ".".to_string(),
        base_commit: "HEAD".to_string(),
        branch: "b".to_string(),
        status: crate::flow::schema::types::RunStatus::Running,
        tasks,
        final_verify: vec!["cargo check".to_string()],
        plan_summary: "summary".to_string(),
    }
}

#[test]
fn test_validate_revised_contract_accepts_valid() {
    let task = sample_task("T01", Assignee::Worker);
    let state = sample_run_state(task.clone());
    let config = sample_config();

    let mut revised = task.clone();
    revised.objective = "revised objective".to_string();

    assert!(is_revised_contract_valid(&revised, &state, &config));
}

#[test]
fn test_validate_revised_contract_rejects_exceeded_limits() {
    let task = sample_task("T01", Assignee::Worker);
    let state = sample_run_state(task.clone());
    let mut config = sample_config();
    config.workers[0].limits.max_est_changed_lines = 20;

    let mut revised = task.clone();
    revised.est_changed_lines = 100;

    assert!(!is_revised_contract_valid(&revised, &state, &config));
}

#[test]
fn test_validate_split_subtasks_accepts_valid() {
    let task = sample_task("T01", Assignee::Worker);
    let state = sample_run_state(task.clone());
    let config = sample_config();

    let sub1 = sample_task("T01.1", Assignee::Worker);
    let sub2 = sample_task("T01.2", Assignee::Worker);

    let task_state = state.tasks.get("T01").unwrap();
    assert!(are_subtasks_valid(&[sub1, sub2], task_state, &config));
}

#[test]
fn test_validate_split_subtasks_rejects_empty_or_too_deep() {
    let task = sample_task("T01", Assignee::Worker);
    let state = sample_run_state(task.clone());
    let config = sample_config();

    let mut task_state = state.tasks.get("T01").unwrap().clone();
    task_state.split_depth = config.limits.max_split_depth;

    let sub1 = sample_task("T01.1", Assignee::Worker);
    assert!(!are_subtasks_valid(&[sub1], &task_state, &config));
    task_state.split_depth = 0;
    assert!(!are_subtasks_valid(&[], &task_state, &config));
}
