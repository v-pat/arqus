mod common;

use common::*;
use std::fs;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn s13_crash_then_resume() {
    let task1 = single_task("T01", "worker", &["cargo check"], &["src/lib.rs"]);
    let mut task2 = single_task("T02", "worker", &["cargo check"], &["src/lib.rs"]);
    task2["depends_on"] = serde_json::json!(["T01"]);
    let mut task3 = single_task("T03", "worker", &["cargo check"], &["src/lib.rs"]);
    task3["depends_on"] = serde_json::json!(["T02"]);

    let plan = plan_json(vec![task1, task2, task3], vec!["cargo check"]);
    let orch_server = MockServer::start(vec![Reply::sse(sse_text(&[&plan]))]);

    let worker_server = MockServer::start(vec![
        // Task 1 succeeds
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn f1() {}\n",
        )])),
        // Task 2 succeeds
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn f1() {}\npub fn f2() {}\n",
        )])),
        // Task 3 hangs so we can kill the process in flight
        Reply::Hang { secs: 30 },
        // Reply after resume for Task 3
        Reply::sse(sse_text(&[&make_file_edit(
            "src/lib.rs",
            "pub fn f1() {}\npub fn f2() {}\npub fn f3() {}\n",
        )])),
    ]);

    let fixture = FlowTestFixture::new(&orch_server.url, &worker_server.url);

    // Spawn arqus flow run
    let mut child = arqus_command(
        &fixture.project.path,
        &fixture.home.path,
        &orch_server.url,
        true,
        &[
            ("ARQUS_ORCH_KEY", "orch_secret"),
            ("ARQUS_WORKER_KEY", "worker_secret"),
            ("ARQUS_RETRY_BASE_MS", "5"),
            ("ARQUS_MAX_RETRIES", "1"),
        ],
    )
    .args(["flow", "run", "three step flow"])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("spawn arqus flow");

    // Wait until worker server sees 3rd request
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if worker_server.requests().len() >= 3 {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        worker_server.requests().len(),
        3,
        "worker server must see 3rd request before kill"
    );

    // Kill arqus
    child.kill().expect("kill arqus child");
    let _ = child.wait();

    let run_id = fixture.run_id().expect("flow run id must exist on branch");

    // Now run flow resume
    let resume_output = fixture.run_flow_args(&["resume", &run_id], &[]);
    if resume_output.status.code() != Some(0) {
        panic!(
            "flow resume failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout(&resume_output),
            stderr(&resume_output)
        );
    }
    assert_eq!(resume_output.status.code(), Some(0));

    // Assert that accepted tasks T01 and T02 were NOT re-run
    // Total worker requests should be exactly 4 (3 before kill + 1 on resume)
    assert_eq!(
        worker_server.requests().len(),
        4,
        "accepted tasks must not be re-run on resume"
    );

    // Orchestrator was only called once at initial planning, not during resume
    assert_eq!(
        orch_server.requests().len(),
        1,
        "orchestrator should not be re-run during resume"
    );

    // All 3 commits must exist on flow branch
    let commits = fixture.get_commits_on_flow_branch();
    assert_eq!(commits.len(), 3, "expected 3 commits on flow branch");

    // Verify worktree file contents
    let flow_dir = fixture.flow_dir().expect("flow dir exists");
    let lib_rs = fs::read_to_string(flow_dir.join("worktree").join("src/lib.rs")).unwrap();
    assert!(lib_rs.contains("pub fn f1()"));
    assert!(lib_rs.contains("pub fn f2()"));
    assert!(lib_rs.contains("pub fn f3()"));
}

