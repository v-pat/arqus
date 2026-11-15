mod common;

use common::*;
use serde_json::json;
use std::fs;

#[test]
fn s8_planner_returns_invalid_json_then_valid() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let valid_plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&["Here is the plan: { not valid json }"])),
        Reply::sse(sse_text(&[&valid_plan])),
    ]);

    let worker_server = MockServer::start(vec![Reply::sse(sse_text(&[&make_file_edit(
        "src/lib.rs",
        "pub fn s8_done() {}\n",
    )]))]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("implement s8 task");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        orch_server.requests().len(),
        2,
        "orch got 1 initial + 1 repair"
    );
    assert_eq!(worker_server.requests().len(), 1, "worker executed once");

    let commits = fixture.get_commits_on_flow_branch();
    assert_eq!(commits.len(), 1);
}

#[test]
fn s9_plan_violates_worker_limits() {
    // Worker limit max_context_files is 5. Task with 6 files violates it.
    let bad_task = json!({
        "id": "T01",
        "title": "Task 1",
        "category": "single_function",
        "assignee": "worker",
        "objective": "Add helper",
        "context": [
            { "path": "src/main.rs" },
            { "path": "src/lib.rs" },
            { "path": "Cargo.toml" },
            { "path": "src/main.rs" },
            { "path": "src/lib.rs" },
            { "path": "Cargo.toml" }
        ],
        "allowed_writes": ["src/lib.rs"],
        "depends_on": [],
        "constraints": [],
        "acceptance_criteria": [],
        "interface_notes": "",
        "verify": ["cargo check"],
        "est_changed_lines": 5
    });
    let bad_plan = plan_json(vec![bad_task], vec!["cargo check"]);

    let good_task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let good_plan = plan_json(vec![good_task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&bad_plan])),
        Reply::sse(sse_text(&[&good_plan])),
    ]);

    let worker_server = MockServer::start(vec![Reply::sse(sse_text(&[&make_file_edit(
        "src/lib.rs",
        "pub fn s9_done() {}\n",
    )]))]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("implement s9 task");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(orch_server.requests().len(), 2);
    // Request 2 must carry the issues from validation
    let req2 = &orch_server.requests()[1];
    assert!(
        req2.body.contains("exceeds worker limit") || req2.body.contains("context_files"),
        "request 2 should mention validation error: {}",
        req2.body
    );
    assert_eq!(worker_server.requests().len(), 1);
}

#[test]
fn s10_repair_budget_exhausted() {
    let bad_json = "I cannot output JSON today.";
    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[bad_json])),
        Reply::sse(sse_text(&[bad_json])),
        Reply::sse(sse_text(&[bad_json])),
    ]);

    let worker_server = MockServer::start(vec![]);
    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("implement s10 task");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(worker_server.requests().len(), 0);

    let commits = fixture.get_commits_on_flow_branch();
    assert_eq!(commits.len(), 0, "no commits beyond base");

    // Working tree is untouched
    let lib_rs = fs::read_to_string(fixture.project.path.join("src/lib.rs")).unwrap();
    assert_eq!(lib_rs, "// lib\n");
}

#[test]
fn s12_plan_includes_non_allowlisted_verify_command() {
    let marker_path = fixture_temp_path("s12_marker.txt");
    let evil_cmd = format!("touch {}", marker_path.display());

    let task = json!({
        "id": "T01",
        "title": "Task 1",
        "category": "single_function",
        "assignee": "worker",
        "objective": "Run forbidden verify",
        "context": [{ "path": "src/main.rs" }],
        "allowed_writes": ["src/lib.rs"],
        "depends_on": [],
        "constraints": [],
        "acceptance_criteria": [],
        "interface_notes": "",
        "verify": [evil_cmd],
        "est_changed_lines": 5
    });
    let plan = plan_json(vec![task], vec![]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&plan])),
    ]);

    let worker_server = MockServer::start(vec![]);
    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("implement s12 task");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(worker_server.requests().len(), 0);
    assert!(
        !marker_path.exists(),
        "forbidden verify command must never be executed"
    );
}

fn fixture_temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("arqus_test_{}_{}", uuid::Uuid::new_v4(), name))
}
