use std::collections::BTreeMap;

use super::*;
use crate::flow::model::ledger::{Ledger, Role, UsageRecord};
use crate::flow::schema::types::{
    Assignee, Category, FileRef, RunState, RunStatus, TaskContract, TaskState, TaskStatus,
};

fn make_contract(id: &str, category: Category, assignee: Assignee) -> TaskContract {
    TaskContract {
        id: id.to_string(),
        title: format!("Title {id}"),
        category,
        assignee,
        objective: "Test objective".to_string(),
        context: vec![FileRef {
            path: "src/lib.rs".to_string(),
            start_line: None,
            end_line: None,
        }],
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 10,
    }
}

fn make_task_state(contract: TaskContract, status: TaskStatus, executor: Assignee) -> TaskState {
    TaskState {
        contract,
        status,
        executor,
        worker_attempts: 0,
        redesigns: 0,
        takeover_attempts: 0,
        split_depth: 0,
        failures: vec![],
        commit: None,
        changed_files: vec![],
    }
}

fn make_run_state(tasks: Vec<TaskState>) -> RunState {
    let mut task_map = BTreeMap::new();
    for t in tasks {
        task_map.insert(t.contract.id.clone(), t);
    }
    RunState {
        run_id: "run-test-123".to_string(),
        objective: "Test run".to_string(),
        repo: "/repo".to_string(),
        base_commit: "abc1234".to_string(),
        branch: "arqus/flow/run-test-123".to_string(),
        status: RunStatus::Finished,
        tasks: task_map,
        final_verify: vec!["cargo test".to_string()],
        plan_summary: "Summary".to_string(),
    }
}

fn make_usage_record(
    task_id: Option<&str>,
    role: Role,
    model: &str,
    input: u64,
    output: u64,
) -> UsageRecord {
    UsageRecord {
        ts: 1000,
        run_id: "run-test-123".to_string(),
        task_id: task_id.map(|s| s.to_string()),
        role,
        provider: "mock".to_string(),
        model: model.to_string(),
        input,
        output,
        cache_read: 0,
        cache_write: 0,
        estimated: false,
        cost_usd: Some(0.001),
        latency_ms: 50,
        ok: true,
    }
}

#[test]
fn test_task_stat_record_json_line_format() {
    let record = TaskStatRecord {
        model: "qwen3.5:9b".to_string(),
        category: Category::Boilerplate,
        first_attempt_pass: true,
        attempts: 1,
        redesigns: 0,
        takeover: false,
        tokens: 1500,
    };

    let serialized = serde_json::to_string(&record).unwrap();
    assert!(serialized.contains("\"model\":\"qwen3.5:9b\""));
    assert!(serialized.contains("\"category\":\"boilerplate\""));
    assert!(serialized.contains("\"first_attempt_pass\":true"));
    assert!(serialized.contains("\"attempts\":1"));
    assert!(serialized.contains("\"redesigns\":0"));
    assert!(serialized.contains("\"takeover\":false"));
    assert!(serialized.contains("\"tokens\":1500"));

    let deserialized: TaskStatRecord = serde_json::from_str(&serialized).unwrap();
    assert_eq!(record, deserialized);
}

#[test]
fn test_build_task_stats_first_attempt_success() {
    let contract = make_contract("T01", Category::SingleFunction, Assignee::Worker);
    let mut task_state = make_task_state(contract, TaskStatus::Done, Assignee::Worker);
    task_state.worker_attempts = 1;

    let run_state = make_run_state(vec![task_state]);
    let mut ledger = Ledger::new(None);
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            500,
            100,
        ))
        .unwrap();

    let stats = build_task_stats(&run_state, &ledger, "default-worker", "default-orch");
    assert_eq!(stats.len(), 1);
    let stat = &stats[0];
    assert_eq!(stat.model, "qwen3.5:9b");
    assert_eq!(stat.category, Category::SingleFunction);
    assert!(stat.first_attempt_pass);
    assert_eq!(stat.attempts, 1);
    assert_eq!(stat.redesigns, 0);
    assert!(!stat.takeover);
    assert_eq!(stat.tokens, 600);
}

#[test]
fn test_build_task_stats_worker_retry() {
    let contract = make_contract("T01", Category::Tests, Assignee::Worker);
    let mut task_state = make_task_state(contract, TaskStatus::Done, Assignee::Worker);
    task_state.worker_attempts = 2;

    let run_state = make_run_state(vec![task_state]);
    let mut ledger = Ledger::new(None);
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            300,
            50,
        ))
        .unwrap();
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            400,
            60,
        ))
        .unwrap();

    let stats = build_task_stats(&run_state, &ledger, "default-worker", "default-orch");
    assert_eq!(stats.len(), 1);
    let stat = &stats[0];
    assert!(!stat.first_attempt_pass);
    assert_eq!(stat.attempts, 2);
    assert_eq!(stat.redesigns, 0);
    assert!(!stat.takeover);
    assert_eq!(stat.tokens, 810);
}

#[test]
fn test_append_run_stats_to_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let stats_path = temp_dir.path().join("flow").join("stats.jsonl");

    let rec1 = TaskStatRecord {
        model: "qwen3.5:9b".to_string(),
        category: Category::Boilerplate,
        first_attempt_pass: true,
        attempts: 1,
        redesigns: 0,
        takeover: false,
        tokens: 300,
    };
    let rec2 = TaskStatRecord {
        model: "qwen3.5:9b".to_string(),
        category: Category::Tests,
        first_attempt_pass: false,
        attempts: 2,
        redesigns: 0,
        takeover: false,
        tokens: 500,
    };

    append_run_stats(&stats_path, &[rec1.clone(), rec2.clone()]).unwrap();

    let rec3 = TaskStatRecord {
        model: "deepseek-flash".to_string(),
        category: Category::MultiFile,
        first_attempt_pass: false,
        attempts: 3,
        redesigns: 1,
        takeover: true,
        tokens: 1200,
    };
    append_run_stats(&stats_path, std::slice::from_ref(&rec3)).unwrap();

    let content = std::fs::read_to_string(&stats_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3);

    let parsed1: TaskStatRecord = serde_json::from_str(lines[0]).unwrap();
    let parsed2: TaskStatRecord = serde_json::from_str(lines[1]).unwrap();
    let parsed3: TaskStatRecord = serde_json::from_str(lines[2]).unwrap();

    assert_eq!(parsed1, rec1);
    assert_eq!(parsed2, rec2);
    assert_eq!(parsed3, rec3);
}
