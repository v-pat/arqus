use super::*;
use crate::flow::schema::types::{
    Assignee, Category, Plan, RunState, RunStatus, TaskContract, TaskStatus,
};

fn sample_task(id: &str, title: &str) -> TaskContract {
    TaskContract {
        id: id.to_string(),
        title: title.to_string(),
        category: Category::SingleFunction,
        assignee: Assignee::Worker,
        objective: "test objective".to_string(),
        context: vec![],
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 5,
    }
}

#[test]
fn test_find_last_accepted_commit_with_no_done_tasks() {
    let plan = Plan {
        summary: "test plan".to_string(),
        tasks: vec![sample_task("T01", "Task 1")],
        final_verify: vec![],
    };
    let state = RunState::from_plan(
        &plan,
        "run1".to_string(),
        "obj".to_string(),
        "/repo".to_string(),
        "base_commit_123".to_string(),
        "branch".to_string(),
    );

    let last_commit = find_last_accepted_commit(&state, None);
    assert_eq!(last_commit, "base_commit_123");
}

#[test]
fn test_find_last_accepted_commit_with_done_tasks() {
    let plan = Plan {
        summary: "test plan".to_string(),
        tasks: vec![sample_task("T01", "Task 1"), sample_task("T02", "Task 2")],
        final_verify: vec![],
    };
    let mut state = RunState::from_plan(
        &plan,
        "run1".to_string(),
        "obj".to_string(),
        "/repo".to_string(),
        "base_commit_123".to_string(),
        "branch".to_string(),
    );
    if let Some(t1) = state.tasks.get_mut("T01") {
        t1.status = TaskStatus::Done;
        t1.commit = Some("commit_t01".to_string());
    }

    let last_commit = find_last_accepted_commit(&state, None);
    assert_eq!(last_commit, "commit_t01");
}

#[test]
fn test_reset_tasks_for_resume_resets_running_and_redesigning_only() {
    let plan = Plan {
        summary: "test plan".to_string(),
        tasks: vec![
            sample_task("T01", "Task 1"),
            sample_task("T02", "Task 2"),
            sample_task("T03", "Task 3"),
            sample_task("T04", "Task 4"),
        ],
        final_verify: vec![],
    };
    let mut state = RunState::from_plan(
        &plan,
        "run1".to_string(),
        "obj".to_string(),
        "/repo".to_string(),
        "base".to_string(),
        "branch".to_string(),
    );

    // T01 is Done
    if let Some(t) = state.tasks.get_mut("T01") {
        t.status = TaskStatus::Done;
        t.worker_attempts = 1;
    }
    // T02 was Running mid-flight when crash happened
    if let Some(t) = state.tasks.get_mut("T02") {
        t.status = TaskStatus::Running;
        t.worker_attempts = 2;
        t.takeover_attempts = 1;
    }
    // T03 was Redesigning
    if let Some(t) = state.tasks.get_mut("T03") {
        t.status = TaskStatus::Redesigning;
        t.worker_attempts = 2;
    }
    // T04 is Pending
    if let Some(t) = state.tasks.get_mut("T04") {
        t.status = TaskStatus::Pending;
    }

    state.status = RunStatus::Running;

    reset_tasks_for_resume(&mut state);

    assert_eq!(state.status, RunStatus::Running);

    let t1 = state.tasks.get("T01").unwrap();
    assert_eq!(t1.status, TaskStatus::Done);
    assert_eq!(t1.worker_attempts, 1); // Not reset

    let t2 = state.tasks.get("T02").unwrap();
    assert_eq!(t2.status, TaskStatus::Ready);
    assert_eq!(t2.worker_attempts, 0); // Reset
    assert_eq!(t2.takeover_attempts, 0); // Reset

    let t3 = state.tasks.get("T03").unwrap();
    assert_eq!(t3.status, TaskStatus::Ready);
    assert_eq!(t3.worker_attempts, 0); // Reset

    let t4 = state.tasks.get("T04").unwrap();
    assert_eq!(t4.status, TaskStatus::Pending); // Kept Pending
}
