//! Stage 22 safety tests: approvals, the path guard and risky commands,
//! through the real binary and the fake server.

mod support;
use support::*;

use std::fs;

use serde_json::json;

fn tool_result_text(body: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    value["messages"]
        .as_array()
        .and_then(|messages| {
            messages.iter().rev().find_map(|message| {
                if message["role"] == json!("tool") {
                    message["content"].as_str().map(str::to_string)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default()
}

#[test]
fn print_mode_asks_before_writing() {
    let project = TempDir::new("ask_project");
    let home = TempDir::new("ask_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_1",
            "write",
            &json!({"path": "inside.txt", "content": "hi"}),
        )),
        Reply::sse(sse_text(&["cannot write"])),
    ]);

    let out = run_arqus_without_yolo(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "write a file"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        !project.path.join("inside.txt").exists(),
        "nothing may be written without approval"
    );
    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "the model must see the refusal");
    let text = tool_result_text(&requests[1].body);
    assert!(
        text.contains("Approval required. Run arqus with --yolo to allow this."),
        "tool result: {text}"
    );
}

#[test]
fn read_never_asks_for_approval() {
    let project = TempDir::new("read_project");
    let home = TempDir::new("read_home");
    fs::write(project.path.join("notes.txt"), "hello from disk").unwrap();
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_1",
            "read",
            &json!({"path": "notes.txt"}),
        )),
        Reply::sse(sse_text(&["read it"])),
    ]);

    let out = run_arqus_without_yolo(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "read notes"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let requests = fake.requests();
    let text = tool_result_text(&requests[1].body);
    assert!(text.contains("hello from disk"), "tool result: {text}");
    assert!(!text.contains("Approval required"), "tool result: {text}");
}

#[test]
fn write_outside_the_project_is_refused_even_with_yolo() {
    let root = TempDir::new("outside_root");
    let project = root.path.join("project");
    fs::create_dir_all(&project).unwrap();
    let home = TempDir::new("outside_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_1",
            "write",
            &json!({"path": "../outside.txt", "content": "escape"}),
        )),
        Reply::sse(sse_text(&["blocked"])),
    ]);

    let out = run_arqus(
        &project,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "write outside"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        !root.path.join("outside.txt").exists(),
        "the path guard must win over --yolo"
    );
    let requests = fake.requests();
    let text = tool_result_text(&requests[1].body);
    assert!(
        text.contains("outside the working directory"),
        "tool result: {text}"
    );
}

#[test]
fn risky_bash_is_refused_in_auto_mode_without_allow_risky() {
    let project = TempDir::new("risky_project");
    let home = TempDir::new("risky_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_1",
            "bash",
            &json!({"command": "sudo true"}),
        )),
        Reply::sse(sse_text(&["blocked"])),
    ]);

    let out = run_arqus(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "try sudo"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let requests = fake.requests();
    let text = tool_result_text(&requests[1].body);
    assert!(
        text.contains("refused risky command"),
        "tool result: {text}"
    );
}

#[test]
fn plan_mode_blocks_mutating_tools_in_e2e() {
    let project = TempDir::new("plan_project");
    let home = TempDir::new("plan_home");
    fs::write(project.path.join("existing.txt"), "hello plan").unwrap();
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_1",
            "read",
            &json!({"path": "existing.txt"}),
        )),
        Reply::sse(sse_tool_call_json(
            "call_2",
            "write",
            &json!({"path": "output.txt", "content": "forbidden"}),
        )),
        Reply::sse(sse_text(&["plan completed"])),
    ]);

    let out = run_arqus_without_yolo(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "--mode", "plan", "-p", "plan the change"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        !project.path.join("output.txt").exists(),
        "mutating tools must not modify filesystem in plan mode"
    );
    let requests = fake.requests();
    assert_eq!(requests.len(), 3);
    // Request 1: system prompt must carry the plan note
    assert!(requests[0]
        .body
        .contains("Plan mode: changes are disabled. Describe the plan instead."));
    // Request 2: read tool result succeeds
    let read_res = tool_result_text(&requests[1].body);
    assert!(
        read_res.contains("hello plan"),
        "read tool result: {read_res}"
    );
    // Request 3: write tool result is refused with PLAN_REFUSAL
    let write_res = tool_result_text(&requests[2].body);
    assert!(
        write_res.contains("Plan mode: changes are disabled. Describe the plan instead."),
        "write tool result: {write_res}"
    );
}

#[test]
fn auto_mode_runs_safe_tools_without_approval_in_e2e() {
    let project = TempDir::new("auto_project");
    let home = TempDir::new("auto_home");
    let fake = FakeLlm::start(vec![
        Reply::sse(sse_tool_call_json(
            "call_1",
            "write",
            &json!({"path": "auto.txt", "content": "automated success"}),
        )),
        Reply::sse(sse_text(&["done"])),
    ]);

    let out = run_arqus_without_yolo(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "--mode", "auto", "-p", "write a file"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        project.path.join("auto.txt").exists(),
        "auto mode must execute without prompting"
    );
    assert_eq!(
        fs::read_to_string(project.path.join("auto.txt")).unwrap(),
        "automated success"
    );
}

#[test]
fn invalid_mode_flag_exits_2() {
    let project = TempDir::new("invalid_mode_project");
    let home = TempDir::new("invalid_mode_home");
    let fake = FakeLlm::start(vec![]);

    let out = run_arqus_without_yolo(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "--mode", "unknown_mode", "-p", "hello"],
        &[],
    );

    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown mode 'unknown_mode': expected ask, auto, or plan"));
}
