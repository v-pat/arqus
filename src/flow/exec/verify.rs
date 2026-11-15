//! Scope checking, changed lines limits, and verification command execution.

use std::path::Path;
use std::process::Command;

use crate::flow::errors::FlowError;
use crate::flow::exec::sandbox::{check_command_allowed, run_command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyResult {
    pub passed: bool,
    pub failed_command: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub timed_out: bool,
}

/// Enforces that worktree modifications strictly adhere to `allowed_writes`,
/// contain no external/parent path escapes, and do not exceed `max_changed_lines_hard`.
pub fn check_scope(
    worktree: &Path,
    allowed_writes: &[String],
    max_changed_lines_hard: u32,
) -> Result<(), FlowError> {
    for path in allowed_writes {
        if path.starts_with('/') || path.contains("..") {
            return Err(FlowError::ScopeViolation(format!(
                "path '{path}' in allowed_writes is outside repository or contains '..'"
            )));
        }
    }

    let status_output = Command::new("git")
        .args(["status", "--porcelain=v1", "-z"])
        .current_dir(worktree)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to run git status: {err}")))?;

    if !status_output.status.success() {
        let err_msg = String::from_utf8_lossy(&status_output.stderr);
        return Err(FlowError::Git(format!("git status failed: {err_msg}")));
    }

    let raw_status = status_output.stdout;
    let parts: Vec<&[u8]> = raw_status.split(|&b| b == 0).collect();
    let mut changed_paths = Vec::new();

    let mut idx = 0;
    while idx < parts.len() {
        let part = parts[idx];
        if part.len() < 3 {
            idx += 1;
            continue;
        }
        let code = &part[..2];
        let path_bytes = &part[3..];
        let path = String::from_utf8_lossy(path_bytes).to_string();
        changed_paths.push(path);

        if code.starts_with(b"R") || code.starts_with(b"C") {
            idx += 1;
            if idx < parts.len() && !parts[idx].is_empty() {
                changed_paths.push(String::from_utf8_lossy(parts[idx]).to_string());
            }
        }
        idx += 1;
    }

    for changed in &changed_paths {
        if !allowed_writes.iter().any(|allowed| allowed == changed) {
            return Err(FlowError::ScopeViolation(format!(
                "file '{changed}' modified or created outside allowed_writes"
            )));
        }
    }

    let diff_output = Command::new("git")
        .args(["diff", "--numstat", "HEAD"])
        .current_dir(worktree)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to run git diff: {err}")))?;

    let mut total_lines: u32 = 0;
    if diff_output.status.success() {
        let stdout = String::from_utf8_lossy(&diff_output.stdout);
        for line in stdout.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                if let Ok(added) = tokens[0].parse::<u32>() {
                    total_lines = total_lines.saturating_add(added);
                }
                if let Ok(deleted) = tokens[1].parse::<u32>() {
                    total_lines = total_lines.saturating_add(deleted);
                }
            }
        }
    }

    for p in &changed_paths {
        let full = worktree.join(p);
        let is_untracked = Command::new("git")
            .args(["ls-files", "--error-unmatch", p])
            .current_dir(worktree)
            .output()
            .map(|out| !out.status.success())
            .unwrap_or(false);

        if is_untracked && full.is_file() {
            if let Ok(content) = std::fs::read_to_string(&full) {
                total_lines = total_lines.saturating_add(content.lines().count() as u32);
            }
        }
    }

    if total_lines > max_changed_lines_hard {
        return Err(FlowError::ScopeViolation(format!(
            "changed lines ({total_lines}) exceeds max_changed_lines_hard limit ({max_changed_lines_hard})"
        )));
    }

    Ok(())
}

/// Runs a sequence of verify commands against the worktree in the sandbox.
/// Fails fast on the first failing command.
pub async fn run_verify_commands(
    worktree: &Path,
    commands: &[String],
    allowed_commands: &[String],
    passthrough_env_keys: &[String],
    timeout_secs: u64,
) -> Result<VerifyResult, FlowError> {
    for cmd in commands {
        let tokens = check_command_allowed(cmd, allowed_commands, worktree, None)?;
        let output = run_command(&tokens, worktree, passthrough_env_keys, timeout_secs).await?;

        if output.timed_out {
            return Ok(VerifyResult {
                passed: false,
                failed_command: Some(cmd.clone()),
                exit_code: None,
                stdout_tail: output.stdout,
                stderr_tail: output.stderr,
                timed_out: true,
            });
        }

        if output.exit_code != Some(0) {
            return Ok(VerifyResult {
                passed: false,
                failed_command: Some(cmd.clone()),
                exit_code: output.exit_code,
                stdout_tail: output.stdout,
                stderr_tail: output.stderr,
                timed_out: false,
            });
        }
    }

    Ok(VerifyResult {
        passed: true,
        failed_command: None,
        exit_code: Some(0),
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        timed_out: false,
    })
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
