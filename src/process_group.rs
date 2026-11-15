//! Process group configuration and cleanup helpers.

use std::process::Stdio;

/// Configure a Command to run in its own process group.
#[cfg(unix)]
pub fn configure_process_group(cmd: &mut tokio::process::Command) {
    cmd.process_group(0);
}

#[cfg(not(unix))]
pub fn configure_process_group(_cmd: &mut tokio::process::Command) {}

/// Kill a process group with `kill -KILL -<pid>`.
pub fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    {
        let target = format!("-{pid}");
        let _ = std::process::Command::new("kill")
            .arg("-KILL")
            .arg(&target)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

/// RAII guard to kill a process group when dropped unless disarmed.
#[derive(Debug, Default)]
pub struct GroupGuard {
    pub pid: Option<u32>,
}

impl GroupGuard {
    pub fn new(pid: Option<u32>) -> Self {
        Self { pid }
    }

    pub fn disarm(&mut self) {
        self.pid = None;
    }

    pub fn kill(&mut self) {
        if let Some(pid) = self.pid.take() {
            kill_process_group(pid);
        }
    }
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
#[path = "process_group_tests.rs"]
mod tests;
