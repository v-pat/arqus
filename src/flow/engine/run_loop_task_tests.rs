use super::*;
use crate::flow::schema::types::{Category, TaskContract, TaskState};
use std::collections::BTreeMap;

#[test]
fn test_run_loop_task_types() {
    let task = TaskContract {
        id: "T01".to_string(),
        title: "T01".to_string(),
        category: Category::SingleFunction,
        assignee: Assignee::Worker,
        objective: "obj".to_string(),
        context: vec![],
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 5,
    };
    let mut tasks = BTreeMap::new();
    tasks.insert(
        "T01".to_string(),
        TaskState {
            contract: task,
            status: TaskStatus::Running,
            executor: Assignee::Worker,
            worker_attempts: 1,
            redesigns: 0,
            takeover_attempts: 0,
            split_depth: 0,
            failures: vec![],
            commit: None,
            changed_files: vec![],
        },
    );
    let state = RunState {
        run_id: "r1".to_string(),
        objective: "obj".to_string(),
        repo: ".".to_string(),
        base_commit: "HEAD".to_string(),
        branch: "b".to_string(),
        status: crate::flow::schema::types::RunStatus::Running,
        tasks,
        final_verify: vec![],
        plan_summary: "s".to_string(),
    };
    assert_eq!(state.tasks.len(), 1);
}
