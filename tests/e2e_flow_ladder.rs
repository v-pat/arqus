mod common;

use common::*;
use serde_json::json;

#[test]
fn s2_worker_fails_verify_passes_on_attempt_2() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![Reply::sse(sse_text(&[&plan]))]);
    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "syntax error;\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn ok() {}\n",
        )])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("retry verify");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(worker_server.requests().len(), 2);
    assert_eq!(orch_server.requests().len(), 1); // planner only

    let req2_body = &worker_server.requests()[1].body;
    assert!(req2_body.contains("verify_failed") || req2_body.contains("verify command failed"));
    assert_eq!(fixture.get_commits_on_flow_branch().len(), 1);
}

#[test]
fn s3_worker_exhausts_attempts_redesign_revises_worker_succeeds() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let revised_task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let redesign_reply = json!({
        "action": "revise",
        "reason": "clarified signature",
        "revised": revised_task
    })
    .to_string();

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&redesign_reply])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "syntax err 1;\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "syntax err 2;\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn revised_ok() {}\n",
        )])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("revise ladder");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(orch_server.requests().len(), 2); // 1 planner + 1 redesign
    assert_eq!(worker_server.requests().len(), 3);
    assert_eq!(fixture.get_commits_on_flow_branch().len(), 1);
}

#[test]
fn s4_redesign_splits_the_task() {
    let t01 = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let mut t02 = single_task("T02", "worker", &["cargo check"], &["src/lib.rs"]);
    t02["depends_on"] = json!(["T01"]);
    let plan = plan_json(vec![t01, t02], vec!["cargo check"]);

    let sub1 = single_task("T01.1", "worker", &["cargo check"], &["src/lib.rs"]);
    let sub2 = single_task("T01.2", "worker", &["cargo check"], &["src/lib.rs"]);
    let split_reply = json!({
        "action": "split",
        "reason": "task too large",
        "split": [sub1, sub2]
    })
    .to_string();

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&split_reply])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "err1;\n")])),
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "err2;\n")])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn s1() {}\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn s1() {}\npub fn s2() {}\n",
        )])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn s1() {}\npub fn s2() {}\npub fn t2() {}\n",
        )])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("split ladder");

    assert_eq!(output.status.code(), Some(0));
    let state_file = fixture.flow_dir().unwrap().join("state.json");
    let state_str = std::fs::read_to_string(state_file).expect("read state.json");
    let state_json: serde_json::Value = serde_json::from_str(&state_str).unwrap();
    assert_eq!(state_json["tasks"]["T01"]["status"], "superseded");
    assert_eq!(state_json["tasks"]["T01.1"]["status"], "done");
    assert_eq!(state_json["tasks"]["T01.2"]["status"], "done");
    assert_eq!(state_json["tasks"]["T02"]["status"], "done");
}

#[test]
fn s5_redesign_chooses_takeover_orchestrator_succeeds() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let takeover_decision = json!({
        "action": "takeover",
        "reason": "beyond worker capability"
    })
    .to_string();

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&takeover_decision])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn takeover_ok() {}\n",
        )])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "w1 err;\n")])),
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "w2 err;\n")])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("takeover ladder");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        worker_server.requests().len(),
        2,
        "worker server not called after takeover"
    );
    assert_eq!(orch_server.requests().len(), 3); // plan, redesign, takeover

    let bodies = fixture.get_commit_bodies_on_flow_branch();
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].to_lowercase().contains("orchestrator"));
}

#[test]
fn s6_max_redesigns_reached_automatic_takeover() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&plan])),
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn direct_takeover() {}\n",
        )])),
    ]);

    let worker_server = MockServer::start(vec![
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "fail1;\n")])),
        Reply::sse(sse_text(&[&make_file_edit("src/lib.rs", "fail2;\n")])),
    ]);

    let fixture = FlowTestFixture::with_limits(
        &orch_server.url,
        &worker_server.url,
        json!({ "max_redesigns": 0 }),
    );
    let output = fixture.run_flow("max redesigns ladder");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(orch_server.requests().len(), 2); // planner + takeover, no redesign!
    assert_eq!(worker_server.requests().len(), 2);
}
