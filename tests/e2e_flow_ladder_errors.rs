mod common;

use common::*;
use serde_json::json;

#[test]
fn s7_takeover_fails_task_failed_dependents_skipped() {
    let mut t01 = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let mut t02 = single_task("T02", "worker", &["cargo check"], &["src/lib.rs"]);
    t02["depends_on"] = json!(["T01"]);
    let t03 = single_task("T03", "worker", &["cargo check"], &["src/other.rs"]);
    t01["allowed_writes"] = json!(["src/lib.rs"]);
    let plan = plan_json(vec![t01, t02, t03], vec!["cargo check"]);

    let takeover_dec = json!({ "action": "takeover", "reason": "go to takeover" }).to_string();

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&takeover_dec])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "bad takeover 1;\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "bad takeover 2;\n",
        )])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "w err 1;\n")])),
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "w err 2;\n")])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/other.rs",
            "// other ok\n",
        )])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("takeover fail ladder");

    assert_ne!(
        output.status.code(),
        Some(0),
        "must exit non-zero when task fails"
    );
    let state_file = fixture.flow_dir().unwrap().join("state.json");
    let state_str = std::fs::read_to_string(state_file).expect("read state.json");
    let state_json: serde_json::Value = serde_json::from_str(&state_str).unwrap();
    assert_eq!(state_json["tasks"]["T01"]["status"], "failed");
    assert_eq!(state_json["tasks"]["T02"]["status"], "skipped");
    assert_eq!(state_json["tasks"]["T03"]["status"], "done");
}

#[test]
fn s11_worker_edits_file_outside_allowed_writes() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[
            &json!({"action": "takeover", "reason": "to"}).to_string()
        ])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/forbidden.rs",
            "// outside\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/forbidden.rs",
            "// outside\n",
        )])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit(
            "src/forbidden.rs",
            "// outside\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/forbidden.rs",
            "// outside\n",
        )])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("scope check");

    assert_ne!(output.status.code(), Some(0));
    let state_file = fixture.flow_dir().unwrap().join("state.json");
    let state_str = std::fs::read_to_string(state_file).expect("read state.json");
    assert!(state_str.contains("scope_violation"));
    assert!(!fixture
        .project
        .path
        .join(".arqus/flow")
        .join("worktree")
        .join("src/forbidden.rs")
        .exists());
}

#[test]
fn s17_provider_returns_500_on_every_retry() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[
            &json!({"action": "takeover", "reason": "to"}).to_string()
        ])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn ok() {}\n",
        )])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::status(500, "{\"error\": \"internal error\"}"),
        Reply::status(500, "{\"error\": \"internal error\"}"),
        Reply::status(500, "{\"error\": \"internal error\"}"),
        Reply::status(500, "{\"error\": \"internal error\"}"),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("provider 500");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(worker_server.requests().len(), 4);
    let state_file = fixture.flow_dir().unwrap().join("state.json");
    let state_str = std::fs::read_to_string(state_file).expect("read state.json");
    assert!(state_str.contains("provider_error"));
}

#[test]
fn s19_worker_outputs_cannot_complete() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[
            &json!({"action": "takeover", "reason": "to"}).to_string()
        ])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn ok() {}\n",
        )])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&["CANNOT_COMPLETE: unclear requirement\n"])),
        Reply::sse(sse_text(&["CANNOT_COMPLETE: still unclear\n"])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("cannot complete");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(worker_server.requests().len(), 2);
    let state_file = fixture.flow_dir().unwrap().join("state.json");
    let state_str = std::fs::read_to_string(state_file).expect("read state.json");
    assert!(state_str.contains("declined"));
}
