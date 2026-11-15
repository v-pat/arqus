use super::*;
use crate::flow::schema::types::{Assignee, Category};

fn make_contract(id: &str, depends_on: Vec<&str>) -> TaskContract {
    TaskContract {
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
    }
}

fn sample_split_state() -> RunState {
    let mut map = BTreeMap::new();
    // T00 (Done) -> T01 (Redesigning) -> T02 (Pending)
    map.insert(
        "T00".to_string(),
        TaskState {
            contract: make_contract("T00", vec![]),
            status: TaskStatus::Done,
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
    map.insert(
        "T01".to_string(),
        TaskState {
            contract: make_contract("T01", vec!["T00"]),
            status: TaskStatus::Redesigning,
            executor: Assignee::Worker,
            worker_attempts: 2,
            redesigns: 1,
            takeover_attempts: 0,
            split_depth: 0,
            failures: vec![],
            commit: None,
            changed_files: vec![],
        },
    );
    map.insert(
        "T02".to_string(),
        TaskState {
            contract: make_contract("T02", vec!["T01"]),
            status: TaskStatus::Pending,
            executor: Assignee::Worker,
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
        run_id: "split-run".to_string(),
        objective: "Test split".to_string(),
        repo: "/tmp/repo".to_string(),
        base_commit: "0123456789abcdef".to_string(),
        branch: "arqus/flow/split-run".to_string(),
        status: RunStatus::Running,
        tasks: map,
        final_verify: vec!["cargo check".to_string()],
        plan_summary: "summary".to_string(),
    }
}

#[test]
fn test_split_supersedes_old_task() {
    let mut state = sample_split_state();
    let subtasks = vec![
        make_contract("T01.1", vec![]),
        make_contract("T01.2", vec![]),
    ];

    state
        .replace_with_subtasks("T01", subtasks)
        .expect("replace_with_subtasks");

    assert_eq!(state.tasks["T01"].status, TaskStatus::Superseded);
}

#[test]
fn test_split_subtask_ids_follow_old_n() {
    let mut state = sample_split_state();
    // Subtasks can be given with blank or partial IDs, or provided with <old>.N
    let subtasks = vec![
        make_contract("temp1", vec![]),
        make_contract("temp2", vec![]),
    ];

    state
        .replace_with_subtasks("T01", subtasks)
        .expect("replace_with_subtasks");

    assert!(state.tasks.contains_key("T01.1"));
    assert!(state.tasks.contains_key("T01.2"));
    assert_eq!(state.tasks["T01.1"].contract.id, "T01.1");
    assert_eq!(state.tasks["T01.2"].contract.id, "T01.2");
}

#[test]
fn test_split_subtasks_inherit_old_dependencies() {
    let mut state = sample_split_state();
    // T01 depended on T00.
    let subtasks = vec![
        make_contract("T01.1", vec![]),
        make_contract("T01.2", vec![]),
    ];

    state
        .replace_with_subtasks("T01", subtasks)
        .expect("replace_with_subtasks");

    // Both subtasks must have inherited T00 as dependency
    assert!(state.tasks["T01.1"]
        .contract
        .depends_on
        .contains(&"T00".to_string()));
    assert!(state.tasks["T01.2"]
        .contract
        .depends_on
        .contains(&"T00".to_string()));
}

#[test]
fn test_split_subtasks_increment_split_depth() {
    let mut state = sample_split_state();
    state.tasks.get_mut("T01").unwrap().split_depth = 0;

    let subtasks = vec![
        make_contract("T01.1", vec![]),
        make_contract("T01.2", vec![]),
    ];

    state
        .replace_with_subtasks("T01", subtasks)
        .expect("replace_with_subtasks");

    assert_eq!(state.tasks["T01.1"].split_depth, 1);
    assert_eq!(state.tasks["T01.2"].split_depth, 1);
}

#[test]
fn test_split_rewires_dependents_to_all_independent_subtasks() {
    let mut state = sample_split_state();
    // T02 originally depended on T01.
    // T01.1 and T01.2 are independent.
    let subtasks = vec![
        make_contract("T01.1", vec![]),
        make_contract("T01.2", vec![]),
    ];

    state
        .replace_with_subtasks("T01", subtasks)
        .expect("replace_with_subtasks");

    let t02_deps = &state.tasks["T02"].contract.depends_on;
    assert!(!t02_deps.contains(&"T01".to_string()));
    assert!(t02_deps.contains(&"T01.1".to_string()));
    assert!(t02_deps.contains(&"T01.2".to_string()));
}

#[test]
fn test_split_rewires_dependents_to_leaf_subtask_in_chain() {
    let mut state = sample_split_state();
    // T01 is split into a chain: T01.1 -> T01.2 (T01.2 depends on T01.1).
    // The leaf subtask is T01.2 (no other subtask depends on it).
    let subtasks = vec![
        make_contract("T01.1", vec![]),
        make_contract("T01.2", vec!["T01.1"]),
    ];

    state
        .replace_with_subtasks("T01", subtasks)
        .expect("replace_with_subtasks");

    let t02_deps = &state.tasks["T02"].contract.depends_on;
    assert!(!t02_deps.contains(&"T01".to_string()));
    assert!(!t02_deps.contains(&"T01.1".to_string()));
    assert!(t02_deps.contains(&"T01.2".to_string()));
}

#[test]
fn test_split_subtask_ready_when_old_deps_done() {
    let mut state = sample_split_state();
    // T00 is Done.
    // Subtask T01.1 depends only on T00, so it should become Ready!
    // Subtask T01.2 depends on T01.1, so it should remain Pending!
    let subtasks = vec![
        make_contract("T01.1", vec![]),
        make_contract("T01.2", vec!["T01.1"]),
    ];

    state
        .replace_with_subtasks("T01", subtasks)
        .expect("replace_with_subtasks");

    assert_eq!(state.tasks["T01.1"].status, TaskStatus::Ready);
    assert_eq!(state.tasks["T01.2"].status, TaskStatus::Pending);
}
