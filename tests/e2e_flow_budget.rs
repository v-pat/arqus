mod common;

use common::*;
use serde_json::json;
use std::process::Command;

#[test]
fn s14_cloud_budget_exceeded_mid_run() {
    let task1 = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let mut task2 = single_task("T02", "orchestrator", &["cargo check"], &["src/lib.rs"]);
    task2["depends_on"] = json!(["T01"]);

    let plan = plan_json(vec![task1, task2], vec!["cargo check"]);
    let orch_server = MockServer::start(vec![
        // Planner reply
        Reply::sse(sse_text(&[&plan])),
    ]);

    let worker_server = MockServer::start(vec![
        // Task 1 succeeds
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn f1() {}\n",
        )])),
    ]);

    // Set max_cloud_tokens_per_run tiny (e.g. 10 tokens) so that planner tokens trip the budget
    // before the orchestrator takeover runs
    let limits = json!({
        "max_cloud_tokens_per_run": 10
    });

    let fixture = FlowTestFixture::with_limits(&orch_server.url, &worker_server.url, limits);
    let output = fixture.run_flow("run two tasks with tight budget");

    assert_eq!(output.status.code(), Some(1));
    let err_msg = stderr(&output);
    assert!(
        err_msg.contains("limits.max_cloud_tokens_per_run"),
        "stderr should name limits.max_cloud_tokens_per_run:\n{err_msg}"
    );

    let flow_dir = fixture.flow_dir().expect("flow dir must exist");
    let state_json =
        std::fs::read_to_string(flow_dir.join("state.json")).expect("state.json exists");
    assert!(
        state_json.contains("\"budget_stopped\""),
        "state status must be budget_stopped, got: {state_json}"
    );

    // No uncommitted changes in worktree
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(flow_dir.join("worktree"))
        .output()
        .expect("git status");
    assert!(
        status.stdout.is_empty(),
        "worktree must have no uncommitted changes: {}",
        String::from_utf8_lossy(&status.stdout)
    );
}
