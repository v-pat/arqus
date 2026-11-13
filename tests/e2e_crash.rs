//! Crash safety and corrupt-session recovery, black box against the fake server.
//! No network and no real key: every run gets its own temp project and ARQUS_HOME.

mod support;
use support::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return out;
    };
    for entry in read {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(jsonl_files(&path));
        } else if path.extension().map(|x| x == "jsonl").unwrap_or(false) {
            out.push(path);
        }
    }
    out
}

fn find_session(home: &Path) -> Option<PathBuf> {
    jsonl_files(&home.join("sessions")).into_iter().next()
}

fn only_session(home: &Path) -> PathBuf {
    find_session(home).expect("expected exactly one session file")
}

fn tool_message(body: &Value) -> &Value {
    body["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .find(|m| m["role"] == json!("tool"))
        .expect("a tool message in the request")
}

#[test]
fn sigkill_during_a_tool_call_resumes_with_a_synthetic_result() {
    let project = TempDir::new("crash_project");
    let home = TempDir::new("crash_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_sleep",
            "bash",
            &json!({"command": "sleep 30"}),
        )),
        Reply::sse(sse_text(&["resumed fine"])),
    ]);

    let mut child = arqus_command(
        &project.path,
        &home.path,
        &fake.url,
        true,
        &[("ARQUS_MAX_OUTPUT_TOKENS", "300")],
    )
    .args(["-p", "run the sleep tool"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("spawn arqus");

    // Wait until the assistant tool call is on disk, then kill the process.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut saw_call = false;
    while Instant::now() < deadline {
        if let Some(path) = find_session(&home.path) {
            if let Ok(text) = fs::read_to_string(path) {
                if text.contains("\"tool_call\"") && text.contains("\"bash\"") {
                    saw_call = true;
                    break;
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(saw_call, "the tool call never reached the session file");
    child.kill().expect("SIGKILL arqus");
    child.wait().expect("reap arqus");

    let session = fs::read_to_string(only_session(&home.path)).unwrap();
    assert!(
        !session.contains("\"tool_result\""),
        "the killed run should not have a tool result"
    );

    let second = run_arqus(
        &project.path,
        &home.path,
        &fake.url,
        &["-c", "-p", "what happened?"],
        &[],
    );
    assert_eq!(second.status.code(), Some(0), "stderr: {}", stderr(&second));

    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "unexpected extra requests");
    let body = requests[1].json();
    let tool = tool_message(&body);
    assert_eq!(tool["tool_call_id"], json!("call_sleep"));
    let content = tool["content"].as_str().expect("tool content is a string");
    assert!(
        content.contains("interrupted"),
        "expected the synthetic interrupted result, got: {content}"
    );
}

#[test]
fn corrupt_middle_line_opens_with_a_warning_on_stderr() {
    let project = TempDir::new("corrupt_project");
    let home = TempDir::new("corrupt_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_text(&["first answer"])),
        Reply::sse(sse_text(&["second answer"])),
    ]);

    let first = run_arqus(&project.path, &home.path, &fake.url, &["-p", "one"], &[]);
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));

    let path = only_session(&home.path);
    let text = fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(|line| line.to_string()).collect();
    assert!(lines.len() >= 3, "expected header plus entries");
    lines.insert(2, "{this is not json".to_string());
    fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();

    let second = run_arqus(
        &project.path,
        &home.path,
        &fake.url,
        &["-c", "-p", "two"],
        &[],
    );
    assert_eq!(second.status.code(), Some(0), "stderr: {}", stderr(&second));
    assert!(
        stderr(&second).contains("skipping corrupt line"),
        "stderr: {}",
        stderr(&second)
    );
}
