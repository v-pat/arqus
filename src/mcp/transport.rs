//! Stdio transport for MCP servers: spawns child process with piped stdin/stdout.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    pid: Option<u32>,
}

impl StdioTransport {
    pub fn spawn(
        command: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
        cwd: &Path,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(unix)]
        cmd.process_group(0);

        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "failed to spawn MCP server `{command}` in {}",
                cwd.display()
            )
        })?;

        let pid = child.id();
        let stdin = child
            .stdin
            .take()
            .context("failed to open stdin for MCP child process")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to open stdout for MCP child process")?;

        // Drain stderr in background to avoid pipe buffer deadlocks.
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    crate::debug::log("mcp_stderr", serde_json::json!({ "line": line }));
                }
            });
        }

        let lines = BufReader::new(stdout).lines();

        Ok(Self {
            child,
            stdin,
            lines,
            pid,
        })
    }

    /// Send a JSON line to server's stdin.
    pub async fn send_line(&mut self, json_str: &str) -> Result<()> {
        self.stdin
            .write_all(json_str.as_bytes())
            .await
            .context("failed writing to MCP server stdin")?;
        self.stdin
            .write_all(b"\n")
            .await
            .context("failed writing newline to MCP server stdin")?;
        self.stdin
            .flush()
            .await
            .context("failed flushing MCP server stdin")?;
        Ok(())
    }

    /// Read next non-empty line from server's stdout.
    pub async fn read_line(&mut self) -> Result<Option<String>> {
        loop {
            match self.lines.next_line().await {
                Ok(Some(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    return Ok(Some(trimmed.to_string()));
                }
                Ok(None) => return Ok(None),
                Err(err) => {
                    return Err(anyhow::anyhow!(
                        "error reading from MCP server stdout: {err}"
                    ))
                }
            }
        }
    }

    /// Check if child process has exited.
    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>> {
        self.child
            .try_wait()
            .context("error checking MCP child status")
    }

    /// Kill the child process and its process group.
    pub fn kill(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid {
            let target = format!("-{pid}");
            let _ = std::process::Command::new("kill")
                .arg("-KILL")
                .arg(&target)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = self.child.start_kill();
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        self.kill();
    }
}
