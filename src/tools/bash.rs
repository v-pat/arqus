//! The `bash` tool: run `sh -c` in the working directory.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use super::{arg_str, arg_usize, ToolOutput};

pub const DEFAULT_TIMEOUT_SECS: usize = 120;
pub const MAX_OUTPUT_BYTES: usize = 50_000;

pub async fn run(args: &Value, cwd: &Path) -> ToolOutput {
    let command = match arg_str(args, "command") {
        Ok(command) => command,
        Err(err) => return ToolOutput::error(err),
    };
    let timeout_secs = match arg_usize(args, "timeout_secs", DEFAULT_TIMEOUT_SECS) {
        Ok(secs) => secs.max(1),
        Err(err) => return ToolOutput::error(err),
    };

    // `kill_on_drop` kills the child when the timeout drops the future.
    let child = match Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return ToolOutput::error(format!(
                "could not start command in {}: {err}",
                cwd.display()
            ))
        }
    };

    let waited = tokio::time::timeout(
        Duration::from_secs(timeout_secs as u64),
        child.wait_with_output(),
    )
    .await;

    let output = match waited {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => return ToolOutput::error(format!("could not run command: {err}")),
        Err(_) => {
            return ToolOutput::error(format!(
                "command timed out after {timeout_secs} seconds and was killed"
            ))
        }
    };

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    let mut text = tail(&combined);
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }

    match output.status.code() {
        Some(code) => {
            text.push_str(&format!("exit code: {code}"));
            if code == 0 {
                ToolOutput::ok(text)
            } else {
                ToolOutput::error(text)
            }
        }
        None => {
            text.push_str("exit code: unknown (terminated by signal)");
            ToolOutput::error(text)
        }
    }
}

/// Keep the last `MAX_OUTPUT_BYTES`, on a char boundary, with a marker.
fn tail(text: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text.to_string();
    }
    let mut start = text.len() - MAX_OUTPUT_BYTES;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    format!("[output truncated]\n{}", &text[start..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cwd() -> std::path::PathBuf {
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
    async fn timeout_kills_the_process() {
        let started = std::time::Instant::now();
        let out = run(&json!({"command": "sleep 5", "timeout_secs": 1}), &cwd()).await;
        assert!(out.is_error);
        assert!(out.text.contains("timed out"), "{}", out.text);
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    #[test]
    fn tail_keeps_end() {
        let text = format!("start\n{}end", "x".repeat(MAX_OUTPUT_BYTES));
        let out = tail(&text);
        assert!(out.starts_with("[output truncated]"));
        assert!(out.ends_with("end"));
        assert!(out.len() <= MAX_OUTPUT_BYTES + 30);
    }
}
