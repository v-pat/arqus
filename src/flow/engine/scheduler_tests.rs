use super::*;
use crate::flow::schema::types::{
    Assignee, Category, RunState, RunStatus, TaskContract, TaskState, TaskStatus,
};
use std::collections::BTreeMap;

fn make_task(id: &str, status: TaskStatus, depends_on: Vec<&str>) -> (String, TaskState) {
    (
        id.to_string(),
        TaskState {
            contract: TaskContract {
                id: id.to_string(),
                title: format!("Task {id}"),
                category: Category::Boilerplate,
                assignee: Assignee::Worker,
                objective: format!("Objective {id}"),
                context: vec![],
                allowed_writes: vec!["file.rs".to_string()],
                depends_on: depends_on.into_iter().map(String::from).collect(),
                constraints: vec![],
                acceptance_criteria: vec![],
                interface_notes: String::new(),
                verify: vec!["cargo check".to_string()],
                est_changed_lines: 10,
            },
            status,
            executor: Assignee::Worker,
            worker_attempts: 0,
            redesigns: 0,
            takeover_attempts: 0,
            split_depth: 0,
            failures: vec![],
            commit: None,
            changed_files: vec![],
        },
    )
}

fn make_state(tasks: Vec<(String, TaskState)>) -> RunState {
    let mut map = BTreeMap::new();
    for (id, state) in tasks {
        map.insert(id, state);
    }
    RunState {
        run_id: "test-run".to_string(),
        objective: "test objective".to_string(),
        repo: "/tmp/repo".to_string(),
        base_commit: "0123456789abcdef".to_string(),
        branch: "arqus/flow/test-run".to_string(),
        status: RunStatus::Running,
        tasks: map,
        final_verify: vec!["cargo test".to_string()],
        plan_summary: "plan summary".to_string(),
    }
}

#[test]
fn test_scheduler_next_ready_deterministic_lowest_id() {
    let state = make_state(vec![
        make_task("T03", TaskStatus::Ready, vec![]),
        make_task("T01", TaskStatus::Ready, vec![]),
        make_task("T02", TaskStatus::Ready, vec![]),
    ]);
    assert_eq!(Scheduler::next_ready(&state), Some("T01".to_string()));
}

#[test]
fn test_scheduler_respects_dependencies() {
    let state = make_state(vec![
        make_task("T01", TaskStatus::Running, vec![]),
        make_task("T02", TaskStatus::Pending, vec!["T01"]),
    ]);
    assert_eq!(Scheduler::next_ready(&state), None);
}

#[test]
fn test_scheduler_returns_none_when_no_ready_tasks() {
    let state = make_state(vec![
        make_task("T01", TaskStatus::Done, vec![]),
        make_task("T02", TaskStatus::Running, vec!["T01"]),
    ]);
    assert_eq!(Scheduler::next_ready(&state), None);
}

#[test]
fn test_scheduler_topological_execution_order() {
    // T01 -> T02 -> T03
    let mut state = make_state(vec![
        make_task("T01", TaskStatus::Ready, vec![]),
        make_task("T02", TaskStatus::Pending, vec!["T01"]),
        make_task("T03", TaskStatus::Pending, vec!["T02"]),
    ]);

    let first = Scheduler::next_ready(&state).expect("should find T01");
    assert_eq!(first, "T01");

    state.tasks.get_mut("T01").unwrap().status = TaskStatus::Done;
    state.tasks.get_mut("T02").unwrap().status = TaskStatus::Ready;

    let second = Scheduler::next_ready(&state).expect("should find T02");
    assert_eq!(second, "T02");

    state.tasks.get_mut("T02").unwrap().status = TaskStatus::Done;
    state.tasks.get_mut("T03").unwrap().status = TaskStatus::Ready;

    let third = Scheduler::next_ready(&state).expect("should find T03");
    assert_eq!(third, "T03");

    state.tasks.get_mut("T03").unwrap().status = TaskStatus::Done;
    assert_eq!(Scheduler::next_ready(&state), None);
}
