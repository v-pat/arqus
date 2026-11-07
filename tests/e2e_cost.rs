//! Stage 16 end-to-end checks: run limits, runaway protection, token caps,
//! the summary line, and the /cost and /budget commands.

mod support;
use support::*;

use serde_json::json;

/// A tool-call reply whose usage reports `cost` dollars.
fn costly_tool(command: &str, cost: f64) -> Vec<Vec<u8>> {
    let mut chunks = sse_tool_call_json("call_1", "bash", &json!({"command": command}));
    let at = chunks.len() - 2;
    chunks[at] = sse_event(&usage_chunk("tool_calls", 100, 20, cost));
    chunks
}

/// A reply with no text and no tool calls.
fn empty_reply() -> Vec<Vec<u8>> {
    vec![
        sse_event(&json!({"id":"fake-1","object":"chat.completion.chunk",
            "choices":[{"index":0,"delta":{},"finish_reason":"stop"}]})),
        sse_event(&usage_chunk("stop", 10, 0, 0.0001)),
        sse_done(),
    ]
}

#[test]
fn run_cost_limit_stops_before_the_third_request() {
    let project = TempDir::new("cost_limit");
    let home = TempDir::new("cost_home");
    let server = FakeLlm::start(vec![
        Reply::sse(costly_tool("echo 1", 0.30)),
        Reply::sse(costly_tool("echo 2", 0.30)),
        Reply::sse(costly_tool("echo 3", 0.30)),
    ]);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["--no-session", "-p", "go", "--max-cost", "0.5"],
        &[],
    );
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(server.requests().len(), 2);

    let err = stderr(&output);
    assert!(
        err.contains("cost limit reached: $0.60 of $0.50 (limits.max_cost_per_run)"),
        "stderr: {err}"
    );
    assert!(err.contains("[pirs] requests=2"), "stderr: {err}");
}

#[test]
fn repeated_identical_tool_call_stops_after_three() {
    let project = TempDir::new("repeat_call");
    let home = TempDir::new("repeat_home");
    let replies: Vec<Reply> = (0..4)
        .map(|_| {
            Reply::sse(sse_tool_call_json(
                "call_1",
                "bash",
                &json!({"command": "echo hi"}),
            ))
        })
        .collect();
    let server = FakeLlm::start(replies);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["--no-session", "-p", "go"],
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(server.requests().len(), 3);
    assert!(
        stderr(&output).contains("stopped: repeated the same tool call 3 times"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn two_empty_assistant_replies_stop_the_turn() {
    let project = TempDir::new("empty_replies");
    let home = TempDir::new("empty_home");
    let server = FakeLlm::start(vec![
        Reply::sse(empty_reply()),
        Reply::sse(empty_reply()),
        Reply::sse(empty_reply()),
    ]);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["--no-session", "-p", "go"],
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(server.requests().len(), 2);
    assert!(
        stderr(&output).contains("stopped: two assistant messages had no text and no tool calls"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn max_output_tokens_flag_is_sent_as_max_tokens() {
    let project = TempDir::new("max_tokens_flag");
    let home = TempDir::new("max_tokens_home");
    let server = FakeLlm::start(vec![Reply::sse(sse_text(&["ok"]))]);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["--no-session", "-p", "go", "--max-output-tokens", "123"],
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(server.requests()[0].json()["max_tokens"], 123);
}

#[test]
fn max_output_tokens_from_settings_is_sent_too() {
    let project = TempDir::new("max_tokens_settings");
    let home = TempDir::new("max_tokens_settings_home");
    std::fs::create_dir_all(project.path.join(".pirs")).unwrap();
    std::fs::write(
        project.path.join(".pirs/settings.json"),
        r#"{"limits":{"max_output_tokens":77}}"#,
    )
    .unwrap();
    let server = FakeLlm::start(vec![Reply::sse(sse_text(&["ok"]))]);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["--no-session", "-p", "go"],
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(server.requests()[0].json()["max_tokens"], 77);
}

#[test]
fn print_mode_ends_with_the_summary_line() {
    let project = TempDir::new("summary_line");
    let home = TempDir::new("summary_home");
    let server = FakeLlm::start(vec![Reply::sse(sse_text(&["hello"]))]);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["--no-session", "-p", "hi"],
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let err = stderr(&output);
    assert!(
        err.lines()
            .any(|line| line.starts_with("[pirs] requests=1 in=100 out=20 cost=$0.0001 time=")),
        "stderr: {err}"
    );
}

#[test]
fn cost_command_prints_the_session_totals() {
    let project = TempDir::new("cost_command");
    let home = TempDir::new("cost_command_home");
    let server = FakeLlm::start(vec![
        Reply::sse(sse_text(&["first"])),
        Reply::sse(sse_text(&["second"])),
    ]);

    let first = run_pirs(&project.path, &home.path, &server.url, &["-p", "hi"], &[]);
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));

    let second = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["-c", "-p", "/cost"],
        &[],
    );
    assert_eq!(second.status.code(), Some(0), "stderr: {}", stderr(&second));
    let out = stdout(&second);
    assert!(out.contains("requests=1"), "stdout: {out}");
    assert!(out.contains("cost=$0.0001"), "stdout: {out}");
    // /cost must not call the model.
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn budget_prints_the_openrouter_key_numbers() {
    let project = TempDir::new("budget_ok");
    let home = TempDir::new("budget_ok_home");
    let body = r#"{"data":{"limit":20.0,"limit_remaining":12.5,"usage_daily":0.25}}"#;
    let server = FakeLlm::start(vec![Reply::status(200, body)]);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["-p", "/budget"],
        &[("PIRS_PROVIDER", "openrouter")],
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("limit_remaining=$12.5000"),
        "stdout: {}",
        stdout(&output)
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0].path.ends_with("/key"),
        "path: {}",
        requests[0].path
    );
}

#[test]
fn budget_failure_is_one_short_line() {
    let project = TempDir::new("budget_fail");
    let home = TempDir::new("budget_fail_home");
    let server = FakeLlm::start(vec![Reply::status(
        401,
        r#"{"error":{"message":"bad key"}}"#,
    )]);

    let output = run_pirs(
        &project.path,
        &home.path,
        &server.url,
        &["-p", "/budget"],
        &[("PIRS_PROVIDER", "openrouter")],
    );
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("budget: request failed with HTTP 401"),
        "stderr: {}",
        stderr(&output)
    );
}
