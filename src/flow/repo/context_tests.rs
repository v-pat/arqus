use std::collections::BTreeMap;
use std::fs;

use super::*;
use crate::flow::schema::types::{
    Assignee, Category, FileRef, RunState, RunStatus, TaskContract, TaskState, TaskStatus,
};

fn make_test_contract(id: &str, depends_on: Vec<&str>) -> TaskContract {
    TaskContract {
        id: id.to_string(),
        title: "Test Task Title".to_string(),
        category: Category::SingleFunction,
        assignee: Assignee::Worker,
        objective: "Add new capability without breaking existing tests".to_string(),
        context: Vec::new(),
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: depends_on.into_iter().map(String::from).collect(),
        constraints: vec!["Do not use unsafe".to_string()],
        acceptance_criteria: vec!["Feature returns true".to_string()],
        interface_notes: "pub fn do_work() -> bool".to_string(),
        verify: vec!["cargo test".to_string()],
        est_changed_lines: 25,
    }
}

fn make_test_state(tasks: Vec<TaskState>) -> RunState {
    let mut map = BTreeMap::new();
    for t in tasks {
        map.insert(t.contract.id.clone(), t);
    }
    RunState {
        run_id: "test-run".to_string(),
        objective: "Test objective".to_string(),
        repo: ".".to_string(),
        base_commit: "abc1234".to_string(),
        branch: "arqus/flow/test-run".to_string(),
        status: RunStatus::Running,
        tasks: map,
        final_verify: vec!["cargo test".to_string()],
        plan_summary: "Test plan summary".to_string(),
    }
}

#[test]
fn test_render_context_whole_file_and_line_range() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    fs::create_dir_all(repo.join("src")).unwrap();
    let file1_content = (1..=10)
        .map(|i| format!("line {i} in file 1"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let file2_content = (1..=20)
        .map(|i| format!("line {i} in file 2"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    fs::write(repo.join("src/whole.rs"), &file1_content).unwrap();
    fs::write(repo.join("src/range.rs"), &file2_content).unwrap();

    let mut contract = make_test_contract("T01", vec![]);
    contract.context = vec![
        FileRef {
            path: "src/whole.rs".to_string(),
            start_line: None,
            end_line: None,
        },
        FileRef {
            path: "src/range.rs".to_string(),
            start_line: Some(5),
            end_line: Some(8),
        },
    ];

    let state = make_test_state(vec![]);
    let rendered = render_task_context(repo, &contract, &state, None).expect("render context");

    // Whole file should have lines 1..10
    assert!(rendered.contains("### FILE: src/whole.rs"));
    assert!(rendered.contains("line 1 in file 1"));
    assert!(rendered.contains("line 10 in file 1"));

    // Range should have (lines 5-8 of 20) and lines 5..8, but NOT lines 1-4 or 9-20
    assert!(rendered.contains("### FILE: src/range.rs (lines 5-8 of 20)"));
    assert!(rendered.contains("line 5 in file 2"));
    assert!(rendered.contains("line 8 in file 2"));
    assert!(!rendered.contains("line 1 in file 2"));
    assert!(!rendered.contains("line 9 in file 2"));
}

#[test]
fn test_render_context_missing_file_typed_error() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    let mut contract = make_test_contract("T01", vec![]);
    contract.context = vec![FileRef {
        path: "nonexistent/file.rs".to_string(),
        start_line: None,
        end_line: None,
    }];

    let state = make_test_state(vec![]);
    let res = render_task_context(repo, &contract, &state, None);

    match res {
        Err(FlowError::MissingContextFile { path }) => {
            assert_eq!(path, "nonexistent/file.rs");
        }
        other => panic!("Expected MissingContextFile, got {other:?}"),
    }
}

#[test]
fn test_render_context_new_file_listed_as_new() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/existing.rs"), "pub fn existing() {}\n").unwrap();

    let mut contract = make_test_contract("T01", vec![]);
    contract.allowed_writes = vec![
        "src/existing.rs".to_string(),
        "src/brand_new.rs".to_string(),
    ];

    let state = make_test_state(vec![]);
    let rendered = render_task_context(repo, &contract, &state, None).expect("render context");

    assert!(rendered.contains("src/existing.rs"));
    assert!(rendered.contains("src/brand_new.rs (new file)"));
}

#[test]
fn test_render_context_upstream_summaries_from_state() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    let dep_contract = TaskContract {
        id: "T01".to_string(),
        title: "Setup database migrations".to_string(),
        category: Category::Boilerplate,
        assignee: Assignee::Worker,
        objective: "Create initial migration scripts".to_string(),
        context: vec![],
        allowed_writes: vec!["migrations/001.sql".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: "".to_string(),
        verify: vec!["cargo test".to_string()],
        est_changed_lines: 30,
    };

    let dep_state = TaskState {
        contract: dep_contract,
        status: TaskStatus::Done,
        executor: Assignee::Worker,
        worker_attempts: 1,
        redesigns: 0,
        takeover_attempts: 0,
        split_depth: 0,
        failures: vec![],
        commit: Some("c0ffee1".to_string()),
        changed_files: vec![
            "migrations/001.sql".to_string(),
            "src/schema.rs".to_string(),
        ],
    };

    let contract = make_test_contract("T02", vec!["T01"]);
    let state = make_test_state(vec![dep_state]);

    let rendered = render_task_context(repo, &contract, &state, None).expect("render context");

    assert!(rendered.contains("T01"));
    assert!(rendered.contains("Setup database migrations"));
    assert!(rendered.contains("migrations/001.sql"));
    assert!(rendered.contains("src/schema.rs"));
}

#[test]
fn test_render_context_over_budget_fails_with_largest_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    fs::create_dir_all(repo.join("src")).unwrap();
    // small file: 100 chars (~25 tokens)
    fs::write(repo.join("src/small.rs"), "a".repeat(100)).unwrap();
    // huge file: 4,000 chars (~1,000 tokens)
    fs::write(repo.join("src/huge.rs"), "b".repeat(4000)).unwrap();

    let mut contract = make_test_contract("T01", vec![]);
    contract.context = vec![
        FileRef {
            path: "src/small.rs".to_string(),
            start_line: None,
            end_line: None,
        },
        FileRef {
            path: "src/huge.rs".to_string(),
            start_line: None,
            end_line: None,
        },
    ];

    let state = make_test_state(vec![]);
    // Budget is 500 tokens, but huge.rs alone is ~1,000 tokens
    let res = render_task_context(repo, &contract, &state, Some(500));

    match res {
        Err(FlowError::BudgetExceeded(msg)) => {
            assert!(
                msg.contains("src/huge.rs"),
                "error message must name the largest file 'src/huge.rs': {msg}"
            );
        }
        other => panic!("Expected BudgetExceeded naming largest file, got {other:?}"),
    }
}

#[test]
fn test_render_context_byte_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/file_a.rs"), "fn a() {}\n").unwrap();
    fs::write(repo.join("src/file_b.rs"), "fn b() {}\n").unwrap();

    let mut contract = make_test_contract("T01", vec![]);
    contract.context = vec![
        FileRef {
            path: "src/file_a.rs".to_string(),
            start_line: None,
            end_line: None,
        },
        FileRef {
            path: "src/file_b.rs".to_string(),
            start_line: None,
            end_line: None,
        },
    ];

    let state = make_test_state(vec![]);

    let r1 = render_task_context(repo, &contract, &state, None).expect("render 1");
    let r2 = render_task_context(repo, &contract, &state, None).expect("render 2");

    assert_eq!(r1, r2);
    assert_eq!(r1.as_bytes(), r2.as_bytes());
}
