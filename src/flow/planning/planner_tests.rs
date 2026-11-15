use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

use super::Planner;
use crate::ai::{Llm, StreamEvent};
use crate::flow::config::defaults::default_flow_settings;
use crate::flow::config::types::ResolvedFlowConfig;
use crate::flow::model::ledger::Ledger;
use crate::types::{Block, Message, StopReason, Usage};

pub(super) fn test_config() -> ResolvedFlowConfig {
    let defaults = default_flow_settings();
    let mut limits = defaults.limits.unwrap();
    limits.max_planning_repairs = 2;
    limits.max_file_read_rounds = 2;
    ResolvedFlowConfig {
        orchestrator: defaults.orchestrator.unwrap(),
        workers: defaults.workers.unwrap(),
        routing: defaults.routing.unwrap(),
        limits,
        sandbox: defaults.sandbox.unwrap(),
        orchestrator_key: "orch-key".to_string(),
        worker_keys: BTreeMap::new(),
    }
}

pub(super) fn mock_llm(responses: Vec<String>) -> (Llm, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    for resp in responses {
        queue.lock().unwrap().push_back(vec![
            StreamEvent::TextDelta(resp.clone()),
            StreamEvent::Done(Message::Assistant {
                blocks: vec![Block::Text { text: resp }],
                provider: "mock".to_string(),
                model: "mock-model".to_string(),
                usage: Some(Usage {
                    input: 100,
                    output: 50,
                    cache_read: 0,
                    cache_write: 0,
                    cost: None,
                }),
                stop: StopReason::Stop,
            }),
        ]);
    }
    (Llm::MockCapture(queue, Arc::clone(&seen)), seen)
}

pub(super) fn sample_plan_json() -> String {
    serde_json::json!({
        "type": "plan",
        "summary": "Sample plan",
        "tasks": [{
            "id": "T01",
            "title": "Initial setup",
            "category": "single_function",
            "assignee": "worker",
            "objective": "Implement foo",
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
    .to_string()
}

#[tokio::test]
async fn test_planner_valid_plan_directly() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "// lib\n").unwrap();

    let (llm, _seen) = mock_llm(vec![sample_plan_json()]);
    let config = test_config();
    let mut ledger = Ledger::new(None);
    let base_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];

    let mut planner = Planner {
        llm: &llm,
        config: &config,
        repo_path: dir.path(),
        base_files: &base_files,
        repo_map: "src/main.rs\n  fn main\n",
        capability_sheet: "Worker: local-9b",
        ledger: &mut ledger,
        run_id: "test-run",
    };

    let plan = planner.plan("Implement foo").await.unwrap();
    assert_eq!(plan.summary, "Sample plan");
    assert_eq!(plan.tasks.len(), 1);
    assert_eq!(plan.tasks[0].id, "T01");
}

#[tokio::test]
async fn test_planner_need_files_round_then_plan() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "// lib\n").unwrap();

    let need_files_reply = serde_json::json!({
        "type": "need_files",
        "files": [{ "path": "src/main.rs" }]
    })
    .to_string();

    let (llm, seen) = mock_llm(vec![need_files_reply, sample_plan_json()]);
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
    assert_eq!(seen.lock().unwrap().len(), 2, "must have made 2 calls");
}

#[tokio::test]
async fn test_planner_need_files_content_truncated_at_60kb() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    let large_content = "x".repeat(70 * 1024);
    fs::write(dir.path().join("src/large.rs"), &large_content).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "// lib\n").unwrap();

    let need_files_reply = serde_json::json!({
        "type": "need_files",
        "files": [{ "path": "src/large.rs" }]
    })
    .to_string();

    let (llm, seen) = mock_llm(vec![need_files_reply, sample_plan_json()]);
    let config = test_config();
    let mut ledger = Ledger::new(None);
    let base_files = vec![
        "src/large.rs".to_string(),
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
    ];

    let mut planner = Planner {
        llm: &llm,
        config: &config,
        repo_path: dir.path(),
        base_files: &base_files,
        repo_map: "src/large.rs\n",
        capability_sheet: "Worker: local-9b",
        ledger: &mut ledger,
        run_id: "test-run",
    };

    let plan = planner.plan("Inspect large file").await.unwrap();
    assert_eq!(plan.tasks.len(), 1);

    let recorded = seen.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    let second_req = &recorded[1];
    assert!(second_req.contains("truncated at 60 KB"));
}

#[tokio::test]
async fn test_planner_exceeding_max_file_read_rounds_forces_plan_request() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "// lib\n").unwrap();

    let need_files_reply = serde_json::json!({
        "type": "need_files",
        "files": [{ "path": "src/main.rs" }]
    })
    .to_string();

    let (llm, seen) = mock_llm(vec![
        need_files_reply.clone(),
        need_files_reply.clone(),
        need_files_reply.clone(),
        sample_plan_json(),
    ]);
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
    assert_eq!(recorded.len(), 4);
    assert!(recorded[3].contains("maximum number of file read rounds"));
}
