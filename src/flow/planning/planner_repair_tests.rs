use std::fs;
use tempfile::tempdir;

use super::tests::{mock_llm, sample_plan_json, test_config};
use super::Planner;
use crate::flow::errors::FlowError;
use crate::flow::model::ledger::Ledger;

#[tokio::test]
async fn test_planner_invalid_json_is_repaired() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "// lib\n").unwrap();

    let bad_json = "Here is your plan: { this is not json }";
    let (llm, seen) = mock_llm(vec![bad_json.to_string(), sample_plan_json()]);
    let config = test_config();
    let mut ledger = Ledger::new(None);
    let base_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];

    let mut planner = Planner {
        llm: &llm,
        config: &config,
        repo_path: dir.path(),
        base_files: &base_files,
        repo_map: "src/main.rs\n",
        capability_sheet: "Worker: local-9b",
        ledger: &mut ledger,
        run_id: "test-run",
    };

    let plan = planner.plan("Implement foo").await.unwrap();
    assert_eq!(plan.tasks.len(), 1);
    let recorded = seen.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    assert!(recorded[1].contains("JSON"));
}

#[tokio::test]
async fn test_planner_invalid_plan_issues_fed_back_and_fixed() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "// lib\n").unwrap();

    let invalid_plan = serde_json::json!({
        "type": "plan",
        "summary": "Bad plan",
        "tasks": [{
            "id": "invalid_id",
            "title": "Title",
            "category": "single_function",
            "assignee": "worker",
            "objective": "Objective",
            "context": [{ "path": "src/main.rs" }],
            "allowed_writes": ["src/lib.rs"],
            "depends_on": [],
            "constraints": [],
            "acceptance_criteria": [],
            "interface_notes": "",
            "verify": ["cargo test"],
            "est_changed_lines": 10
        }],
        "final_verify": ["cargo test"]
    })
    .to_string();

    let (llm, seen) = mock_llm(vec![invalid_plan, sample_plan_json()]);
    let config = test_config();
    let mut ledger = Ledger::new(None);
    let base_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];

    let mut planner = Planner {
        llm: &llm,
        config: &config,
        repo_path: dir.path(),
        base_files: &base_files,
        repo_map: "src/main.rs\n",
        capability_sheet: "Worker: local-9b",
        ledger: &mut ledger,
        run_id: "test-run",
    };

    let plan = planner.plan("Implement foo").await.unwrap();
    assert_eq!(plan.tasks[0].id, "T01");
    let recorded = seen.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    assert!(recorded[1].contains("task id 'invalid_id' invalid"));
}

#[tokio::test]
async fn test_planner_exhausted_repair_budget_returns_planning_failed() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "// lib\n").unwrap();

    let bad_json = "{ bad json }".to_string();
    let (llm, _seen) = mock_llm(vec![bad_json.clone(), bad_json.clone(), bad_json]);
    let config = test_config();
    let mut ledger = Ledger::new(None);
    let base_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];

    let mut planner = Planner {
        llm: &llm,
        config: &config,
        repo_path: dir.path(),
        base_files: &base_files,
        repo_map: "src/main.rs\n",
        capability_sheet: "Worker: local-9b",
        ledger: &mut ledger,
        run_id: "test-run",
    };

    let result = planner.plan("Implement foo").await;
    match result {
        Err(FlowError::PlanningFailed(msg)) => {
            assert!(msg.contains("exhausted"));
        }
        other => panic!("expected PlanningFailed, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_planner_stops_when_cloud_budget_exceeded() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();

    let (llm, _seen) = mock_llm(vec![sample_plan_json()]);
    let mut config = test_config();
    config.limits.max_cloud_tokens_per_run = 100;

    let mut ledger = Ledger::new(None);
    let _ = ledger.record(crate::flow::model::ledger::UsageRecord {
        ts: 100,
        run_id: "test".into(),
        task_id: None,
        role: crate::flow::model::ledger::Role::Planner,
        provider: "openai".into(),
        model: "m".into(),
        input: 200,
        output: 50,
        cache_read: 0,
        cache_write: 0,
        estimated: false,
        cost_usd: None,
        latency_ms: 10,
        ok: true,
    });

    let base_files = vec!["src/main.rs".to_string()];
    let mut planner = Planner {
        llm: &llm,
        config: &config,
        repo_path: dir.path(),
        base_files: &base_files,
        repo_map: "src/main.rs\n",
        capability_sheet: "Worker: local-9b",
        ledger: &mut ledger,
        run_id: "test-run",
    };

    let result = planner.plan("Implement foo").await;
    match result {
        Err(FlowError::BudgetExceeded(msg)) => {
            assert!(msg.contains("limits.max_cloud_tokens_per_run"));
        }
        other => panic!("expected BudgetExceeded, got: {:?}", other),
    }
}
