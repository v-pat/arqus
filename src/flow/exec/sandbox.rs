//! Sandbox execution environment, command allowlist checking, and process isolation.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::flow::errors::FlowError;
use crate::process_group::{configure_process_group, GroupGuard};

pub const MAX_OUTPUT_CHARS: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Validates that a command matches the allowlist and does not trigger risky patterns.
/// An exact leading-token match is accepted. Lookalikes and risky commands are rejected.
pub fn check_command_allowed(
    cmd: &str,
    allowed_commands: &[String],
    cwd: &Path,
    user_home: Option<&Path>,
) -> Result<Vec<String>, FlowError> {
    let tokens = shlex::split(cmd).ok_or_else(|| {
        FlowError::VerificationFailed(format!(
            "failed to parse command: unclosed quotes in '{cmd}'"
        ))
    })?;

    if tokens.is_empty() {
        return Err(FlowError::VerificationFailed(
            "empty command is not allowed".to_string(),
        ));
    }

    let mut is_allowed = false;
    for allowed in allowed_commands {
        if let Some(allowed_tokens) = shlex::split(allowed) {
            if !allowed_tokens.is_empty()
                && tokens.len() >= allowed_tokens.len()
                && tokens[..allowed_tokens.len()] == allowed_tokens[..]
            {
                is_allowed = true;
                break;
            }
        }
    }

    if !is_allowed {
        return Err(FlowError::VerificationFailed(format!(
            "command '{cmd}' is not in sandbox allowed_commands"
        )));
    }

    if let Some(reason) = crate::safety::is_risky(cmd, cwd, user_home) {
        return Err(FlowError::VerificationFailed(format!(
            "command '{cmd}' rejected: risky pattern detected ({reason})"
        )));
    }

    Ok(tokens)
}

/// Runs a command in an isolated environment with scrubbed variables,
/// process group cleanup on timeout, and tail-truncated output.
pub async fn run_command(
    argv: &[String],
    cwd: &Path,
    passthrough_env_keys: &[String],
    timeout_secs: u64,
) -> Result<CommandOutput, FlowError> {
    if argv.is_empty() {
        return Err(FlowError::VerificationFailed(
            "no command specified".to_string(),
        ));
    }

    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    configure_process_group(&mut cmd);

    cmd.env_clear();
    for key in passthrough_env_keys {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    let child = cmd.spawn().map_err(|err| {
        FlowError::VerificationFailed(format!("failed to spawn '{}': {err}", argv[0]))
    })?;

    let mut group = GroupGuard::new(child.id());

    let wait_res =
        tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;

    match wait_res {
        Ok(Ok(output)) => {
            group.disarm();
            let raw_stdout = String::from_utf8_lossy(&output.stdout);
            let raw_stderr = String::from_utf8_lossy(&output.stderr);
            Ok(CommandOutput {
                exit_code: output.status.code(),
                stdout: truncate_tail(&raw_stdout, MAX_OUTPUT_CHARS),
                stderr: truncate_tail(&raw_stderr, MAX_OUTPUT_CHARS),
                timed_out: false,
            })
        }
        Ok(Err(err)) => {
            group.kill();
            Err(FlowError::VerificationFailed(format!(
                "error waiting for child: {err}"
            )))
        }
        Err(_) => {
            group.kill();
            Ok(CommandOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: format!("command timed out after {timeout_secs}s and was killed"),
                timed_out: true,
            })
        }
    }
}

fn truncate_tail(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        text.to_string()
    } else {
        text.chars().skip(char_count - max_chars).collect()
    }
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
