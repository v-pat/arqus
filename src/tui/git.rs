//! Git facts for the footer: branch and dirty flag.
//!
//! The parser is pure so the layout can be tested without a repository.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    pub branch: String,
    pub dirty: bool,
}

/// Turn the output of `rev-parse --abbrev-ref HEAD` and `status --porcelain`
/// into facts. `None` means "not a repository".
pub fn parse(branch_out: &str, status_out: &str) -> Option<GitInfo> {
    let branch = branch_out.lines().next().unwrap_or("").trim();
    if branch.is_empty() {
        return None;
    }
    Some(GitInfo {
        branch: branch.to_string(),
        dirty: !status_out.trim().is_empty(),
    })
}

/// Run the two commands in `cwd`. Any failure means "no git facts".
pub fn read(cwd: &Path) -> Option<GitInfo> {
    let branch = Command::new("git")
        .current_dir(cwd)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !branch.status.success() {
        return None;
    }
    let status = Command::new("git")
        .current_dir(cwd)
        .args(["status", "--porcelain"])
        .output()
        .ok()?;
    parse(
        &String::from_utf8_lossy(&branch.stdout),
        &String::from_utf8_lossy(&status.stdout),
    )
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
