//! Black-box tests: run the real `pirs` binary against a scripted fake server.
//! No network and no real key: every run gets its own temp project and PIRS_HOME.

mod support;
use support::*;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

fn tool_message(body: &Value) -> &Value {
    body["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .find(|m| m["role"] == json!("tool"))
        .expect("a tool message in the request")
}

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

#[test]
fn plain_reply_streams_text_and_sends_tools() {
    let project = TempDir::new("plain_project");
    let home = TempDir::new("plain_home");
    let fake = FakeLlm::start(vec![Reply::sse(sse_text(&["Hel", "lo"]))]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Hello"), "stdout: {}", stdout(&out));

    let requests = fake.requests();
    assert_eq!(requests.len(), 1, "unexpected extra requests");
    let req = &requests[0];
    assert!(
        req.path.ends_with("/chat/completions"),
        "path: {}",
        req.path
    );
    assert_eq!(req.header("authorization"), Some("Bearer test-key-123"));

    let body = req.json();
    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["messages"][0]["role"], json!("system"));
    assert_eq!(
        body["tools"].as_array().map(|tools| tools.len()),
        Some(4),
        "expected the four core tools"
    );
}

#[test]
fn bash_tool_runs_and_result_is_sent_back() {
    let project = TempDir::new("tool_project");
    let home = TempDir::new("tool_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_1",
            "bash",
            &json!({"command": "echo hi > out.txt"}),
        )),
        Reply::sse(sse_text(&["done"])),
    ]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "run the bash tool"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let written = fs::read_to_string(project.path.join("out.txt")).expect("out.txt exists");
    assert_eq!(written.trim(), "hi");

    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "unexpected extra requests");
    let second = requests[1].json();
    let tool = tool_message(&second);
    assert_eq!(tool["tool_call_id"], json!("call_1"));
    let content = tool["content"].as_str().expect("tool content is a string");
    assert!(content.contains("exit code: 0"), "tool content: {content}");
}

#[test]
fn unknown_tool_becomes_an_error_tool_message() {
    let project = TempDir::new("unknown_project");
    let home = TempDir::new("unknown_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json("call_x", "nosuchtool", &json!({}))),
        Reply::sse(sse_text(&["done"])),
    ]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "call a missing tool"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));

    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "unexpected extra requests");
    let second = requests[1].json();
    let tool = tool_message(&second);
    assert_eq!(tool["tool_call_id"], json!("call_x"));
    let content = tool["content"].as_str().expect("tool content is a string");
    assert!(content.contains("unknown tool"), "tool content: {content}");
}

#[test]
fn session_file_is_written_and_continue_replays_history() {
    let project = TempDir::new("session_project");
    let home = TempDir::new("session_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_text(&["first answer"])),
        Reply::sse(sse_text(&["second answer"])),
    ]);

    let first = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["-p", "first prompt"],
        &[],
    );
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));

    let files = jsonl_files(&home.path.join("sessions"));
    assert_eq!(files.len(), 1, "expected exactly one session file");
    let text = fs::read_to_string(&files[0]).expect("read session file");
    let lines: Vec<Value> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("session line is JSON"))
        .collect();
    assert_eq!(lines[0]["type"], json!("session"));
    let roles: Vec<&str> = lines[1..]
        .iter()
        .filter_map(|entry| entry["message"]["role"].as_str())
        .collect();
    assert!(roles.contains(&"user"), "roles: {roles:?}");
    assert!(roles.contains(&"assistant"), "roles: {roles:?}");

    let second = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["-c", "-p", "second prompt"],
        &[],
    );
    assert_eq!(second.status.code(), Some(0), "stderr: {}", stderr(&second));

    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "unexpected extra requests");
    let body = requests[1].json();
    let messages = body["messages"].to_string();
    assert!(
        messages.contains("first prompt"),
        "second request did not replay history: {messages}"
    );
}
