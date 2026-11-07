//! Black-box tests for provider errors, retries, timeouts and exit codes.
//! Every run uses the fake server, a temp project and a temp PIRS_HOME.

mod support;
use support::*;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

fn error_body(message: &str) -> String {
    json!({"error": {"message": message, "code": 500}}).to_string()
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

fn session_text(home: &Path) -> String {
    jsonl_files(&home.join("sessions"))
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect::<Vec<String>>()
        .join("\n")
}

#[test]
fn rate_limited_then_success() {
    let project = TempDir::new("err_429_project");
    let home = TempDir::new("err_429_home");
    let fake = FakeLlm::start(vec![
        Reply::status_with(429, &error_body("slow down"), &[("retry-after", "0")]),
        Reply::sse(sse_text(&["ok after retry"])),
    ]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("ok after retry"));
    assert_eq!(fake.requests().len(), 2, "expected one retry");
    let err = stderr(&out);
    assert!(
        err.contains("rate limited") && err.contains("retrying"),
        "stderr: {err}"
    );
}

#[test]
fn server_errors_twice_then_success() {
    let project = TempDir::new("err_503_project");
    let home = TempDir::new("err_503_home");
    let fake = FakeLlm::start(vec![
        Reply::status_with(503, &error_body("busy"), &[("retry-after", "0")]),
        Reply::status_with(503, &error_body("still busy"), &[("retry-after", "0")]),
        Reply::sse(sse_text(&["finally"])),
    ]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("finally"));
    assert_eq!(fake.requests().len(), 3, "expected two retries");
}

#[test]
fn auth_error_exits_1_without_retry() {
    let project = TempDir::new("err_401_project");
    let home = TempDir::new("err_401_home");
    let fake = FakeLlm::start(vec![Reply::status(401, &error_body("bad key"))]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(fake.requests().len(), 1, "auth must never be retried");
    let err = stderr(&out);
    assert!(err.contains("Authentication failed"), "stderr: {err}");
    assert!(
        !err.contains("test-key-123"),
        "the key must never be printed: {err}"
    );
}

#[test]
fn payment_required_exits_1_without_retry() {
    let project = TempDir::new("err_402_project");
    let home = TempDir::new("err_402_home");
    let fake = FakeLlm::start(vec![Reply::status(402, &error_body("no credits"))]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(fake.requests().len(), 1, "402 must never be retried");
    let err = stderr(&out);
    assert!(err.contains("No credits left"), "stderr: {err}");
}

#[test]
fn bad_request_exits_1_without_retry() {
    let project = TempDir::new("err_400_project");
    let home = TempDir::new("err_400_home");
    let fake = FakeLlm::start(vec![Reply::status(400, &error_body("model not found"))]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[],
    );

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(fake.requests().len(), 1, "400 must never be retried");
    let err = stderr(&out);
    assert!(err.contains("Bad request"), "stderr: {err}");
    assert!(err.contains("model not found"), "stderr: {err}");
}

#[test]
fn connection_refused_exits_1() {
    let project = TempDir::new("err_refused_project");
    let home = TempDir::new("err_refused_home");

    let out = run_pirs(
        &project.path,
        &home.path,
        "http://127.0.0.1:1/v1",
        &["--no-session", "-p", "hi"],
        &[("PIRS_MAX_RETRIES", "1")],
    );

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("Network error"), "stderr: {err}");
}

#[test]
fn idle_timeout_then_success() {
    let project = TempDir::new("err_hang_project");
    let home = TempDir::new("err_hang_home");
    let fake = FakeLlm::start(vec![
        Reply::Hang { secs: 3 },
        Reply::sse(sse_text(&["awake"])),
    ]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[("PIRS_IDLE_TIMEOUT_SECS", "1")],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("awake"));
    assert_eq!(
        fake.requests().len(),
        2,
        "expected one retry after the hang"
    );
    assert!(
        stderr(&out).contains("network error"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn error_after_partial_text_is_not_retried() {
    let project = TempDir::new("err_partial_project");
    let home = TempDir::new("err_partial_home");
    let error_chunk = json!({
        "error": {"message": "Provider disconnected", "code": 502},
        "choices": [{"index": 0, "delta": {}, "finish_reason": "error"}]
    });
    let fake = FakeLlm::start(vec![Reply::sse(vec![
        sse_event(&content_chunk("partial answer")),
        sse_event(&error_chunk),
    ])]);

    let out = run_pirs(&project.path, &home.path, &fake.url, &["-p", "hi"], &[]);

    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(
        fake.requests().len(),
        1,
        "a retry after output could repeat actions"
    );
    assert!(stdout(&out).contains("partial answer"));
    let saved = session_text(&home.path);
    assert!(saved.contains("partial answer"), "session: {saved}");
    assert!(saved.contains("\"stop\":\"error\""), "session: {saved}");
    assert!(
        stderr(&out).contains("Provider disconnected"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn missing_key_exits_2() {
    let project = TempDir::new("err_nokey_project");
    let home = TempDir::new("err_nokey_home");

    // A remote host: a missing key is a configuration error there.
    let out = run_pirs(
        &project.path,
        &home.path,
        "https://api.example.invalid/v1",
        &["--no-session", "-p", "hi"],
        &[("PIRS_API_KEY", "")],
    );

    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("API key"), "stderr: {}", stderr(&out));
}

#[test]
fn empty_model_exits_2() {
    let project = TempDir::new("err_nomodel_project");
    let home = TempDir::new("err_nomodel_home");
    let fake = FakeLlm::start(vec![]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[("PIRS_MODEL", "")],
    );

    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("no model"),
        "stderr: {}",
        stderr(&out)
    );
    assert_eq!(fake.requests().len(), 0);
}

#[test]
fn localhost_without_key_is_allowed() {
    let project = TempDir::new("err_local_project");
    let home = TempDir::new("err_local_home");
    let fake = FakeLlm::start(vec![Reply::sse(sse_text(&["local ok"]))]);

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["--no-session", "-p", "hi"],
        &[("PIRS_API_KEY", "")],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("local ok"));
}

#[test]
fn context_overflow_compacts_then_retries() {
    let project = TempDir::new("err_overflow_project");
    let home = TempDir::new("err_overflow_home");
    let settings = project.path.join(".pirs");
    fs::create_dir_all(&settings).expect("settings folder");
    fs::write(
        settings.join("settings.json"),
        json!({
            "context_window": 100,
            "compaction": {"reserve_tokens": 10, "keep_recent_tokens": 10}
        })
        .to_string(),
    )
    .expect("settings file");
    let fake = FakeLlm::start(vec![
        // First run: one short answer, builds a two-message history.
        Reply::sse(sse_text(&["history answer"])),
        // Second run: overflow, then the summary, then the real answer.
        Reply::status(400, &error_body("context length exceeded")),
        Reply::sse(sse_text(&["short summary of the conversation"])),
        Reply::sse(sse_text(&["after compaction"])),
    ]);
    let first_prompt = "x".repeat(500);
    let second_prompt = "y".repeat(500);

    let first = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["-p", &first_prompt],
        &[],
    );
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));

    let out = run_pirs(
        &project.path,
        &home.path,
        &fake.url,
        &["-c", "-p", &second_prompt],
        &[],
    );

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("after compaction"));
    assert_eq!(
        fake.requests().len(),
        4,
        "history, overflow, summary, answer"
    );
}
