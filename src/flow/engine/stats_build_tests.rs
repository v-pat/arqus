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
        run_id: "run-test-456".to_string(),
        objective: "Test run".to_string(),
        repo: "/repo".to_string(),
        base_commit: "abc1234".to_string(),
        branch: "arqus/flow/run-test-456".to_string(),
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
        run_id: "run-test-456".to_string(),
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
fn test_build_task_stats_redesign_revise() {
    let contract = make_contract("T01", Category::LocalRefactor, Assignee::Worker);
    let mut task_state = make_task_state(contract, TaskStatus::Done, Assignee::Worker);
    task_state.worker_attempts = 2;
    task_state.redesigns = 1;

    let run_state = make_run_state(vec![task_state]);
    let mut ledger = Ledger::new(None);
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            200,
            50,
        ))
        .unwrap();
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Redesign,
            "deepseek-flash",
            400,
            100,
        ))
        .unwrap();
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            250,
            60,
        ))
        .unwrap();

    let stats = build_task_stats(&run_state, &ledger, "default-worker", "default-orch");
    assert_eq!(stats.len(), 1);
    let stat = &stats[0];
    assert!(!stat.first_attempt_pass);
    assert_eq!(stat.attempts, 2);
    assert_eq!(stat.redesigns, 1);
    assert!(!stat.takeover);
    assert_eq!(stat.tokens, 1060);
}

#[test]
fn test_build_task_stats_takeover() {
    let contract = make_contract("T01", Category::MultiFile, Assignee::Worker);
    let mut task_state = make_task_state(contract, TaskStatus::Done, Assignee::Orchestrator);
    task_state.worker_attempts = 2;
    task_state.redesigns = 1;
    task_state.takeover_attempts = 1;

    let run_state = make_run_state(vec![task_state]);
    let mut ledger = Ledger::new(None);
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            200,
            50,
        ))
        .unwrap();
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Takeover,
            "deepseek-flash",
            500,
            150,
        ))
        .unwrap();

    let stats = build_task_stats(&run_state, &ledger, "qwen3.5:9b", "deepseek-flash");
    assert_eq!(stats.len(), 1);
    let stat = &stats[0];
    assert!(!stat.first_attempt_pass);
    assert_eq!(stat.attempts, 3);
    assert_eq!(stat.redesigns, 1);
    assert!(stat.takeover);
    assert_eq!(stat.tokens, 900);
}

#[test]
fn test_build_task_stats_orchestrator_assigned() {
    let contract = make_contract("T01", Category::Other, Assignee::Orchestrator);
    let mut task_state = make_task_state(contract, TaskStatus::Done, Assignee::Orchestrator);
    task_state.takeover_attempts = 1;

    let run_state = make_run_state(vec![task_state]);
    let mut ledger = Ledger::new(None);
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Takeover,
            "deepseek-flash",
            600,
            120,
        ))
        .unwrap();

    let stats = build_task_stats(&run_state, &ledger, "qwen3.5:9b", "deepseek-flash");
    assert_eq!(stats.len(), 1);
    let stat = &stats[0];
    assert_eq!(stat.model, "deepseek-flash");
    assert!(stat.first_attempt_pass);
    assert_eq!(stat.attempts, 1);
    assert_eq!(stat.redesigns, 0);
    assert!(stat.takeover);
    assert_eq!(stat.tokens, 720);
}

#[test]
fn test_build_task_stats_tokens_summed_from_ledger() {
    let contract = make_contract("T01", Category::Boilerplate, Assignee::Worker);
    let mut task_state = make_task_state(contract, TaskStatus::Done, Assignee::Worker);
    task_state.worker_attempts = 1;

    let run_state = make_run_state(vec![task_state]);
    let mut ledger = Ledger::new(None);
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            100,
            50,
        ))
        .unwrap();
    ledger
        .record(make_usage_record(
            Some("T01"),
            Role::Worker,
            "qwen3.5:9b",
            200,
            100,
        ))
        .unwrap();
    ledger
        .record(make_usage_record(
            Some("OTHER"),
            Role::Worker,
            "qwen3.5:9b",
            999,
            999,
        ))
        .unwrap();

    let stats = build_task_stats(&run_state, &ledger, "qwen3.5:9b", "deepseek-flash");
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].tokens, 450);
}
