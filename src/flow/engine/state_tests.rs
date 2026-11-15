use super::*;
use crate::flow::schema::types::{Assignee, Category};

const ALL_STATUSES: [TaskStatus; 8] = [
    TaskStatus::Pending,
    TaskStatus::Ready,
    TaskStatus::Running,
    TaskStatus::Redesigning,
    TaskStatus::Done,
    TaskStatus::Failed,
    TaskStatus::Skipped,
    TaskStatus::Superseded,
];

fn sample_task(id: &str, status: TaskStatus, depends_on: Vec<&str>) -> TaskState {
    TaskState {
        contract: TaskContract {
            id: id.to_string(),
            title: format!("Task {id}"),
            category: Category::Boilerplate,
            assignee: Assignee::Worker,
            objective: format!("Objective {id}"),
            context: vec![],
            allowed_writes: vec!["src/lib.rs".to_string()],
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
    }
}

fn sample_state(tasks: Vec<TaskState>) -> RunState {
    let mut map = BTreeMap::new();
    for t in tasks {
        map.insert(t.contract.id.clone(), t);
    }
    RunState {
        run_id: "test-run".to_string(),
        objective: "Test run".to_string(),
        repo: "/tmp/repo".to_string(),
        base_commit: "0123456789abcdef".to_string(),
        branch: "arqus/flow/test-run".to_string(),
        status: RunStatus::Running,
        tasks: map,
        final_verify: vec!["cargo check".to_string()],
        plan_summary: "summary".to_string(),
    }
}

#[test]
fn test_status_transition_matrix() {
    let allowed: &[(TaskStatus, TaskStatus)] = &[
        (TaskStatus::Pending, TaskStatus::Ready),
        (TaskStatus::Pending, TaskStatus::Skipped),
        (TaskStatus::Ready, TaskStatus::Running),
        (TaskStatus::Running, TaskStatus::Done),
        (TaskStatus::Running, TaskStatus::Redesigning),
        (TaskStatus::Running, TaskStatus::Failed),
        (TaskStatus::Redesigning, TaskStatus::Running),
        (TaskStatus::Redesigning, TaskStatus::Superseded),
        (TaskStatus::Redesigning, TaskStatus::Failed),
    ];

    for &from in &ALL_STATUSES {
        for &to in &ALL_STATUSES {
            let mut task = sample_task("T01", from, vec![]);
            let res = task.transition(to);
            let is_allowed = allowed.contains(&(from, to));
            if is_allowed {
                assert!(
                    res.is_ok(),
                    "expected transition from {from:?} to {to:?} to succeed"
                );
                assert_eq!(task.status, to);
            } else {
                assert!(
                    res.is_err(),
                    "expected transition from {from:?} to {to:?} to fail"
                );
                match res.unwrap_err() {
                    FlowError::InvalidTransition {
                        from: f,
                        to: t,
                        task_id,
                    } => {
                        assert_eq!(f, format!("{from:?}"));
                        assert_eq!(t, format!("{to:?}"));
                        assert_eq!(task_id, "T01");
                    }
                    other => panic!("unexpected error variant: {other:?}"),
                }
            }
        }
    }
}

#[test]
fn test_pending_to_ready_when_dependencies_done() {
    let mut state = sample_state(vec![
        sample_task("T01", TaskStatus::Running, vec![]),
        sample_task("T02", TaskStatus::Pending, vec!["T01"]),
    ]);

    state
        .transition_task("T01", TaskStatus::Done)
        .expect("T01 Done");
    assert_eq!(state.tasks["T01"].status, TaskStatus::Done);
    assert_eq!(state.tasks["T02"].status, TaskStatus::Ready);
}

#[test]
fn test_pending_to_skipped_when_dependency_fails_transitively() {
    let mut state = sample_state(vec![
        sample_task("T01", TaskStatus::Running, vec![]),
        sample_task("T02", TaskStatus::Pending, vec!["T01"]),
        sample_task("T03", TaskStatus::Pending, vec!["T02"]),
        sample_task("T04", TaskStatus::Pending, vec!["T03"]),
    ]);

    state
        .transition_task("T01", TaskStatus::Failed)
        .expect("T01 Failed");
    assert_eq!(state.tasks["T01"].status, TaskStatus::Failed);
    assert_eq!(state.tasks["T02"].status, TaskStatus::Skipped);
    assert_eq!(state.tasks["T03"].status, TaskStatus::Skipped);
    assert_eq!(state.tasks["T04"].status, TaskStatus::Skipped);
}

#[test]
fn test_pending_to_skipped_when_dependency_skipped() {
    let mut state = sample_state(vec![
        sample_task("T01", TaskStatus::Skipped, vec![]),
        sample_task("T02", TaskStatus::Pending, vec!["T01"]),
    ]);

    state.update_dependents();
    assert_eq!(state.tasks["T02"].status, TaskStatus::Skipped);
}

#[test]
fn test_independent_tasks_not_skipped_on_failure() {
    let mut state = sample_state(vec![
        sample_task("T01", TaskStatus::Running, vec![]),
        sample_task("T02", TaskStatus::Pending, vec!["T01"]),
        sample_task("T03", TaskStatus::Ready, vec![]),
    ]);

    state
        .transition_task("T01", TaskStatus::Failed)
        .expect("T01 Failed");
    assert_eq!(state.tasks["T01"].status, TaskStatus::Failed);
    assert_eq!(state.tasks["T02"].status, TaskStatus::Skipped);
    assert_eq!(state.tasks["T03"].status, TaskStatus::Ready);
}

#[test]
fn test_ready_to_running() {
    let mut state = sample_state(vec![sample_task("T01", TaskStatus::Ready, vec![])]);
    state
        .transition_task("T01", TaskStatus::Running)
        .expect("T01 Running");
    assert_eq!(state.tasks["T01"].status, TaskStatus::Running);
}

#[test]
fn test_running_to_done() {
    let mut state = sample_state(vec![
        sample_task("T01", TaskStatus::Running, vec![]),
        sample_task("T02", TaskStatus::Pending, vec!["T01"]),
    ]);
    state
        .transition_task("T01", TaskStatus::Done)
        .expect("T01 Done");
    assert_eq!(state.tasks["T01"].status, TaskStatus::Done);
    assert_eq!(state.tasks["T02"].status, TaskStatus::Ready);
}

#[test]
fn test_running_to_redesigning() {
    let mut state = sample_state(vec![sample_task("T01", TaskStatus::Running, vec![])]);
    state
        .transition_task("T01", TaskStatus::Redesigning)
        .expect("T01 Redesigning");
    assert_eq!(state.tasks["T01"].status, TaskStatus::Redesigning);
}

#[test]
fn test_redesigning_to_running_on_revise() {
    let mut state = sample_state(vec![sample_task("T01", TaskStatus::Redesigning, vec![])]);
    state
        .transition_task("T01", TaskStatus::Running)
        .expect("T01 Running");
    assert_eq!(state.tasks["T01"].status, TaskStatus::Running);
}

#[test]
fn test_state_persistence_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");

    let state = sample_state(vec![
        sample_task("T01", TaskStatus::Done, vec![]),
        sample_task("T02", TaskStatus::Ready, vec!["T01"]),
    ]);

    state.save(&state_path).expect("save state");
    let loaded = RunState::load(&state_path).expect("load state");

    assert_eq!(loaded.run_id, state.run_id);
    assert_eq!(loaded.objective, state.objective);
    assert_eq!(loaded.repo, state.repo);
    assert_eq!(loaded.base_commit, state.base_commit);
    assert_eq!(loaded.branch, state.branch);
    assert_eq!(loaded.status, state.status);
    assert_eq!(loaded.final_verify, state.final_verify);
    assert_eq!(loaded.plan_summary, state.plan_summary);
    assert_eq!(loaded.tasks.len(), 2);
    assert_eq!(loaded.tasks["T01"].status, TaskStatus::Done);
    assert_eq!(loaded.tasks["T02"].status, TaskStatus::Ready);
}

#[test]
fn test_state_persistence_atomic_crash_leaves_original_intact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");

    let state_original = sample_state(vec![sample_task("T01", TaskStatus::Ready, vec![])]);
    state_original.save(&state_path).expect("save original");

    // Simulate an interrupted save: a temp file was left behind before rename
    let temp_interrupted = tmp.path().join(".state.json.arqus-tmp-999-0");
    std::fs::write(&temp_interrupted, b"corrupted partial data").expect("write temp");

    // Original file must be completely untouched and intact
    let loaded = RunState::load(&state_path).expect("load original");
    assert_eq!(loaded.tasks["T01"].status, TaskStatus::Ready);

    // Now write a valid updated state; it should cleanly overwrite state.json
    let mut state_updated = state_original;
    state_updated
        .transition_task("T01", TaskStatus::Running)
        .expect("Running");
    state_updated.save(&state_path).expect("save updated");

    let loaded_updated = RunState::load(&state_path).expect("load updated");
    assert_eq!(loaded_updated.tasks["T01"].status, TaskStatus::Running);
}
