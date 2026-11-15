mod common;

use common::*;
use serde_json::json;
use std::process::Command;

#[test]
fn s1_three_linear_tasks_all_succeed_first_try() {
    let tasks = vec![
        json!({
            "id": "T01",
            "title": "Task 1",
            "category": "single_function",
            "assignee": "worker",
            "objective": "Add helper 1",
            "context": [{ "path": "src/main.rs" }],
            "allowed_writes": ["src/lib.rs"],
            "depends_on": [],
            "constraints": [],
            "acceptance_criteria": [],
            "interface_notes": "",
            "verify": ["cargo check"],
            "est_changed_lines": 5
        }),
        json!({
            "id": "T02",
            "title": "Task 2",
            "category": "single_function",
            "assignee": "worker",
            "objective": "Add helper 2",
            "context": [{ "path": "src/lib.rs" }],
            "allowed_writes": ["src/lib.rs"],
            "depends_on": ["T01"],
            "constraints": [],
            "acceptance_criteria": [],
            "interface_notes": "",
            "verify": ["cargo check"],
            "est_changed_lines": 5
        }),
        json!({
            "id": "T03",
            "title": "Task 3",
            "category": "single_function",
            "assignee": "worker",
            "objective": "Add helper 3",
            "context": [{ "path": "src/lib.rs" }],
            "allowed_writes": ["src/lib.rs"],
            "depends_on": ["T02"],
            "constraints": [],
            "acceptance_criteria": [],
            "interface_notes": "",
            "verify": ["cargo check"],
            "est_changed_lines": 5
        }),
    ];

    let plan = plan_json(tasks, vec!["cargo check"]);
    let orch_server = MockServer::start(vec![Reply::sse(sse_text(&[&plan]))]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn f1() {}\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn f1() {}\npub fn f2() {}\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn f1() {}\npub fn f2() {}\npub fn f3() {}\n",
        )])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("implement linear tasks");

    if output.status.code() != Some(0) {
        panic!(
            "flow run failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout(&output),
            stderr(&output)
        );
    }
    assert_eq!(output.status.code(), Some(0));

    // Assert request counts
    assert_eq!(
        orch_server.requests().len(),
        1,
        "orch server should get 1 request (planner)"
    );
    assert_eq!(
        worker_server.requests().len(),
        3,
        "worker server should get 3 requests"
    );

    // Assert 3 commits in order on the flow branch
    let commits = fixture.get_commits_on_flow_branch();
    assert_eq!(commits.len(), 3, "expected 3 commits on flow branch");
    assert!(commits[0].contains("T03"));
    assert!(commits[1].contains("T02"));
    assert!(commits[2].contains("T01"));

    // Assert base branch working tree is untouched
    let git_status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&fixture.project.path)
        .output()
        .expect("git status");
    assert!(
        git_status.stdout.is_empty(),
        "working tree of base repo must be untouched"
    );
}

#[test]
fn s16_task_assigned_to_orchestrator() {
    let task = single_task("T01", "orchestrator", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn orch_code() {}\n",
        )])),
    ]);

    let worker_server = MockServer::start(vec![]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("orchestrator task");

    assert_eq!(output.status.code(), Some(0));

    // Worker server receives no request
    assert_eq!(
        worker_server.requests().len(),
        0,
        "worker server must receive 0 requests"
    );
    // Orchestrator server receives 2 requests: planner + takeover
    assert_eq!(
        orch_server.requests().len(),
        2,
        "orch server must receive planner + takeover"
    );

    // Commit body names orchestrator as executor
    let bodies = fixture.get_commit_bodies_on_flow_branch();
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].to_lowercase().contains("orchestrator"));
}
