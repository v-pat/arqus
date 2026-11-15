mod common;

use common::*;
use std::fs;

#[test]
fn s15_dirty_repository() {
    let orch_server = MockServer::start(vec![]);
    let worker_server = MockServer::start(vec![]);
    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);

    // Make repo dirty with an untracked file
    fs::write(fixture.project.path.join("untracked.txt"), "dirty").unwrap();

    let output = fixture.run_flow("should fail due to dirty repo");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("clean") || stderr(&output).contains("dirty"),
        "stderr should mention clean/dirty repo: {}",
        stderr(&output)
    );

    // Assert that nothing was created
    assert!(
        fixture.get_flow_branch().is_none(),
        "flow branch should not be created"
    );
    assert!(
        !fixture.project.path.join(".arqus/flow").exists(),
        ".arqus/flow should not be created"
    );
}

#[test]
fn s18_existing_cli_unchanged() {
    let orch_server = MockServer::start(vec![Reply::sse(sse_text(&["hello from mock"]))]);
    let worker_server = MockServer::start(vec![]);
    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);

    // 1. arqus --version
    let ver_out = run_arqus(
        &fixture.project.path,
        &fixture.home.path,
        "",
        &["--version"],
        &[],
    );
    assert_eq!(ver_out.status.code(), Some(0));
    let ver_str = stdout(&ver_out);
    assert!(ver_str.contains("arqus"), "version output: {ver_str}");

    // 2. arqus flow --help
    let help_out = run_arqus(
        &fixture.project.path,
        &fixture.home.path,
        "",
        &["flow", "--help"],
        &[],
    );
    assert_eq!(help_out.status.code(), Some(0));
    let help_str = stdout(&help_out);
    assert!(help_str.contains("run") && help_str.contains("resume") && help_str.contains("report"));

    // 3. arqus -p against a mock
    let p_out = run_arqus(
        &fixture.project.path,
        &fixture.home.path,
        &orch_server.url,
        &["-p", "ping"],
        &[("ARQUS_API_KEY", "test_key")],
    );
    assert_eq!(p_out.status.code(), Some(0));
    assert!(stdout(&p_out).contains("hello from mock"));
}

#[test]
fn s20_key_hygiene() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![Reply::sse(sse_text(&[&plan]))]);
    let worker_server = MockServer::start(vec![Reply::sse(sse_text(&[&make_file_edit(
        "src/lib.rs",
        "pub fn s20_clean() {}\n",
    )]))]);

    let orch_key = "super_secret_orch_key_12345";
    let worker_key = "super_secret_worker_key_67890";

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow_args(
        &["run", "key hygiene test", "--debug"],
        &[
            ("ARQUS_ORCH_KEY", orch_key),
            ("ARQUS_WORKER_KEY", worker_key),
        ],
    );

    assert_eq!(output.status.code(), Some(0));

    let stdout_text = stdout(&output);
    let stderr_text = stderr(&output);
    assert!(!stdout_text.contains(orch_key), "orch key found in stdout");
    assert!(
        !stdout_text.contains(worker_key),
        "worker key found in stdout"
    );
    assert!(!stderr_text.contains(orch_key), "orch key found in stderr");
    assert!(
        !stderr_text.contains(worker_key),
        "worker key found in stderr"
    );

    let flow_dir = fixture.flow_dir().expect("flow dir exists");

    // Check state.json
    let state_file = flow_dir.join("state.json");
    if state_file.exists() {
        let content = fs::read_to_string(&state_file).unwrap();
        assert!(!content.contains(orch_key), "orch key in state.json");
        assert!(!content.contains(worker_key), "worker key in state.json");
    }

    // Check events.jsonl
    let events_file = flow_dir.join("events.jsonl");
    if events_file.exists() {
        let content = fs::read_to_string(&events_file).unwrap();
        assert!(!content.contains(orch_key), "orch key in events.jsonl");
        assert!(!content.contains(worker_key), "worker key in events.jsonl");
    }

    // Check usage.jsonl
    let usage_file = flow_dir.join("usage.jsonl");
    if usage_file.exists() {
        let content = fs::read_to_string(&usage_file).unwrap();
        assert!(!content.contains(orch_key), "orch key in usage.jsonl");
        assert!(!content.contains(worker_key), "worker key in usage.jsonl");
    }

    // Check debug.log
    let debug_log = fixture.home.path.join(".arqus/debug.log");
    if debug_log.exists() {
        let content = fs::read_to_string(&debug_log).unwrap();
        assert!(!content.contains(orch_key), "orch key in debug.log");
        assert!(!content.contains(worker_key), "worker key in debug.log");
    }
}

#[test]
fn test_flow_run_appends_to_stats_jsonl() {
    let task = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let plan = plan_json(vec![task], vec!["cargo check"]);

    let orch_server = MockServer::start(vec![Reply::sse(sse_text(&[&plan]))]);
    let worker_server = MockServer::start(vec![Reply::sse(sse_text(&[&make_file_edit(
        "src/lib.rs",
        "pub fn s_stats() {}\n",
    )]))]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);
    let output = fixture.run_flow("stats run test");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let stats_path = fixture.home.path.join("flow").join("stats.jsonl");
    assert!(
        stats_path.exists(),
        "stats.jsonl should be created at {}",
        stats_path.display()
    );

    let content = fs::read_to_string(&stats_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "expected 1 stat line for 1 task");

    let val: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(val["first_attempt_pass"], true);
    assert_eq!(val["attempts"], 1);
    assert_eq!(val["redesigns"], 0);
    assert_eq!(val["takeover"], false);
    assert!(val["tokens"].as_u64().unwrap() > 0);
}
