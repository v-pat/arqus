//! The `bash` tool: run `sh -c` in its own process group.
//!
//! Rust note: `sh -c "sleep 30 &"` leaves children behind after the shell
//! exits. A process group lets us kill the shell and every child it started
//! with one `kill -KILL -<pid>`; killing only the shell leaks the children.

use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::task::JoinHandle;

use crate::process_group::{configure_process_group, GroupGuard};

use super::{arg_str, arg_usize, truncate_from_end, ToolOutput};

pub const DEFAULT_TIMEOUT_SECS: usize = 120;
pub const MAX_TIMEOUT_SECS: usize = 600;
pub const MAX_OUTPUT_BYTES: usize = 50_000;
pub const MAX_OUTPUT_LINES: usize = 2_000;
/// After the main process exits, read leftover output for at most this long.
const DRAIN_MILLIS: u64 = 200;

pub async fn run(args: &Value, cwd: &Path) -> ToolOutput {
    let command = match arg_str(args, "command") {
        Ok(command) => command,
        Err(err) => return ToolOutput::error(err),
    };
    let timeout_secs = match arg_usize(args, "timeout_secs", DEFAULT_TIMEOUT_SECS) {
        Ok(secs) => secs.clamp(1, MAX_TIMEOUT_SECS),
        Err(err) => return ToolOutput::error(err),
    };

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(&command)
        .current_dir(cwd)
        // /dev/null stdin: a command that waits for input fails at once
        // instead of hanging until the timeout.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("PAGER", "cat")
        .env("GIT_PAGER", "cat")
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .kill_on_drop(true);
    configure_process_group(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            return ToolOutput::error(format!(
                "could not start command in {}: {err}",
                cwd.display()
            ))
        }
    };

    let mut group = GroupGuard::new(child.id());
    let out_buf = Arc::new(Mutex::new(Vec::new()));
    let err_buf = Arc::new(Mutex::new(Vec::new()));
    let mut out_task = tokio::spawn(pump(child.stdout.take(), out_buf.clone()));
    let mut err_task = tokio::spawn(pump(child.stderr.take(), err_buf.clone()));

    let waited = tokio::time::timeout(Duration::from_secs(timeout_secs as u64), child.wait()).await;

    let status = match waited {
        Ok(Ok(status)) => status,
        Ok(Err(err)) => {
            group.kill();
            return ToolOutput::error(format!("could not run command: {err}"));
        }
        Err(_) => {
            group.kill();
            // Reap the shell, but never wait forever if the kill failed.
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            out_task.abort();
            err_task.abort();
            return ToolOutput::error(format!(
                "command timed out after {timeout_secs} seconds and was killed"
            ));
        }
    };

    // A background child can keep the pipes open. Give the pumps a short
    // window, then stop reading and kill the whole group.
    if !drain(&mut out_task, &mut err_task).await {
        group.kill();
        out_task.abort();
        err_task.abort();
    }
    group.disarm();

    let combined = collect(&out_buf, &err_buf);
    let mut text = tail(&combined);
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }

    match status.code() {
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

/// Read a pipe to the end into a shared buffer.
async fn pump<R>(reader: Option<R>, buf: Arc<Mutex<Vec<u8>>>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return;
    };
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buffer(&buf).extend_from_slice(&chunk[..n]),
        }
    }
}

/// Wait at most `DRAIN_MILLIS` for both pumps. False means they are still open.
async fn drain(out_task: &mut JoinHandle<()>, err_task: &mut JoinHandle<()>) -> bool {
    let deadline = Instant::now() + Duration::from_millis(DRAIN_MILLIS);
    while !(out_task.is_finished() && err_task.is_finished()) {
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    true
}

fn buffer(buf: &Arc<Mutex<Vec<u8>>>) -> std::sync::MutexGuard<'_, Vec<u8>> {
    buf.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn collect(out_buf: &Arc<Mutex<Vec<u8>>>, err_buf: &Arc<Mutex<Vec<u8>>>) -> String {
    let mut bytes = buffer(out_buf).clone();
    bytes.extend_from_slice(&buffer(err_buf));
    strip_ansi(&String::from_utf8_lossy(&bytes))
}

/// Remove ANSI escape sequences with a small state machine (no regex crate).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: ESC [ ... one final byte 0x40..=0x7e
            Some('[') => {
                for n in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&n) {
                        break;
                    }
                }
            }
            // OSC: ESC ] ... BEL or ESC \
            Some(']') => loop {
                match chars.next() {
                    None | Some('\x07') => break,
                    Some('\x1b') => {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                    Some(_) => {}
                }
            },
            // Any other two-byte escape: drop both bytes.
            Some(_) | None => {}
        }
    }
    out
}

/// Keep the last `MAX_OUTPUT_BYTES` bytes and the last `MAX_OUTPUT_LINES` lines.
fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut cut = false;
    let mut body = if lines.len() > MAX_OUTPUT_LINES {
        cut = true;
        lines[lines.len() - MAX_OUTPUT_LINES..].join("\n")
    } else {
        text.to_string()
    };
    if body.len() > MAX_OUTPUT_BYTES {
        body = truncate_from_end(&body, MAX_OUTPUT_BYTES).to_string();
        cut = true;
    }
    if cut {
        format!("[output truncated]\n{body}")
    } else {
        body
    }
}

#[cfg(test)]
#[path = "bash_tests.rs"]
mod tests;
