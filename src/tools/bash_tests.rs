//! Tests for the `bash` tool.

use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

use serde_json::json;

use super::*;

fn cwd() -> PathBuf {
    std::env::temp_dir()
}

#[tokio::test]
async fn echo_hi_exits_zero() {
    let out = run(&json!({"command": "echo hi"}), &cwd()).await;
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("hi"), "{}", out.text);
    assert!(out.text.contains("exit code: 0"), "{}", out.text);
}

#[tokio::test]
async fn exit_three_is_error() {
    let out = run(&json!({"command": "exit 3"}), &cwd()).await;
    assert!(out.is_error);
    assert!(out.text.contains("exit code: 3"), "{}", out.text);
}

#[tokio::test]
async fn stdin_is_dev_null_so_cat_does_not_hang() {
    let started = Instant::now();
    let out = run(&json!({"command": "cat", "timeout_secs": 5}), &cwd()).await;
    assert!(!out.is_error, "{}", out.text);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "cat waited for stdin: {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn environment_is_set_for_non_interactive_tools() {
    let command = r#"printf '%s|%s|%s|%s|%s|%s' \
"$GIT_TERMINAL_PROMPT" "$PAGER" "$GIT_PAGER" "$TERM" "$NO_COLOR" "$CI""#;
    let out = run(&json!({"command": command}), &cwd()).await;
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("0|cat|cat|dumb|1|1"), "{}", out.text);
}

#[tokio::test]
async fn ansi_escapes_are_stripped() {
    let out = run(
        &json!({"command": r#"printf '\033[31mred\033[0m plain'"#}),
        &cwd(),
    )
    .await;
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("red plain"), "{}", out.text);
    assert!(!out.text.contains('\x1b'), "{:?}", out.text);
}

#[tokio::test]
async fn background_process_does_not_hang_the_call() {
    let started = Instant::now();
    let out = run(
        &json!({"command": "sleep 30 & echo started", "timeout_secs": 10}),
        &cwd(),
    )
    .await;
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("started"), "{}", out.text);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "took {:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_kills_the_whole_process_group() {
    let started = Instant::now();
    let out = run(
        &json!({"command": "sleep 4242 & wait", "timeout_secs": 1}),
        &cwd(),
    )
    .await;
    assert!(out.is_error);
    assert!(out.text.contains("timed out"), "{}", out.text);
    assert!(started.elapsed() < Duration::from_secs(5));

    // The grandchild sleep must be gone, not just the shell.
    let mut leftover = true;
    for _ in 0..20 {
        if !pgrep_sleep_4242() {
            leftover = false;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!leftover, "`sleep 4242` survived the group kill");
}

#[cfg(unix)]
fn pgrep_sleep_4242() -> bool {
    let output = StdCommand::new("pgrep").args(["-f", "sleep 4242"]).output();
    match output {
        Ok(output) => !output.stdout.is_empty(),
        Err(_) => false, // no pgrep: cannot check, do not fail the suite
    }
}

#[tokio::test]
async fn timeout_secs_above_the_maximum_is_accepted() {
    let out = run(
        &json!({"command": "echo ok", "timeout_secs": 99_999}),
        &cwd(),
    )
    .await;
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("ok"), "{}", out.text);
}

#[test]
fn tail_keeps_the_end() {
    let text = format!("start\n{}end", "x".repeat(MAX_OUTPUT_BYTES));
    let out = tail(&text);
    assert!(out.starts_with("[output truncated]"));
    assert!(out.ends_with("end"));
    assert!(out.len() <= MAX_OUTPUT_BYTES + 30);
}

#[test]
fn tail_keeps_at_most_the_last_2000_lines() {
    let text: String = (0..3000).map(|i| format!("line{i}\n")).collect();
    let out = tail(&text);
    assert!(out.starts_with("[output truncated]"));
    assert_eq!(out.lines().nth(1), Some("line1000"));
    assert_eq!(out.lines().count(), MAX_OUTPUT_LINES + 1);
}

#[test]
fn strip_ansi_handles_colors_and_osc() {
    let text = "\x1b[1;31mred\x1b[0m normal \x1b]0;title\x07tail";
    assert_eq!(strip_ansi(text), "red normal tail");
}
