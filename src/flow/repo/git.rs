//! Git repository management, worktree isolation, and clean state enforcement.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::flow::errors::FlowError;

/// Refuse to run on a dirty repository. Modified and untracked files both count.
pub fn check_clean_repo(repo: &Path) -> Result<(), FlowError> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-z"])
        .current_dir(repo)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to run git status: {err}")))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(FlowError::Git(format!(
            "not a git repository or git status failed: {err_msg}"
        )));
    }

    if !output.stdout.is_empty() {
        return Err(FlowError::Git(
            "repository has uncommitted changes or untracked files; clean repository required"
                .to_string(),
        ));
    }

    Ok(())
}

/// Appends `.arqus/flow/` to `.git/info/exclude` if not already present.
/// Never modifies `.gitignore`.
pub fn ensure_exclude_entry(repo: &Path) -> Result<(), FlowError> {
    let git_dir_output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(repo)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to run git rev-parse: {err}")))?;

    if !git_dir_output.status.success() {
        let err_msg = String::from_utf8_lossy(&git_dir_output.stderr);
        return Err(FlowError::Git(format!(
            "failed to find git directory: {err_msg}"
        )));
    }

    let git_dir_raw = String::from_utf8_lossy(&git_dir_output.stdout);
    let git_dir = repo.join(git_dir_raw.trim());
    let info_dir = git_dir.join("info");
    let exclude_path = info_dir.join("exclude");

    if let Err(err) = std::fs::create_dir_all(&info_dir) {
        return Err(FlowError::Git(format!(
            "failed to create git info folder: {err}"
        )));
    }

    let content = std::fs::read_to_string(&exclude_path).unwrap_or_default();
    if !content.lines().any(|line| line.trim() == ".arqus/flow/") {
        let mut new_content = content;
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(".arqus/flow/\n");
        crate::atomic_write::write_atomic(&exclude_path, new_content.as_bytes())
            .map_err(|err| FlowError::Git(format!("failed to update git info exclude: {err}")))?;
    }

    Ok(())
}

/// Creates branch `arqus/flow/<run_id>` and worktree at `<repo>/.arqus/flow/<run_id>/worktree`.
pub fn create_branch_and_worktree(
    repo: &Path,
    run_id: &str,
    base_commit: Option<&str>,
) -> Result<(String, PathBuf), FlowError> {
    let branch = format!("arqus/flow/{run_id}");
    let worktree = repo
        .join(".arqus")
        .join("flow")
        .join(run_id)
        .join("worktree");

    if let Some(parent) = worktree.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            return Err(FlowError::Git(format!(
                "failed to create run directory: {err}"
            )));
        }
    }

    let mut args = vec![
        "worktree",
        "add",
        "-b",
        &branch,
        worktree.to_str().unwrap_or(""),
    ];
    if let Some(commit) = base_commit {
        args.push(commit);
    }

    let output = Command::new("git")
        .args(&args)
        .current_dir(repo)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to create worktree: {err}")))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(FlowError::Git(format!(
            "git worktree add failed: {err_msg}"
        )));
    }

    Ok((branch, worktree))
}

/// Returns the current commit hash (HEAD) for the repo or worktree.
pub fn get_head_commit(repo_or_worktree: &Path) -> Result<String, FlowError> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_or_worktree)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to get HEAD commit: {err}")))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(FlowError::Git(format!(
            "git rev-parse HEAD failed: {err_msg}"
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Reset and clean worktree to HEAD (`git reset --hard HEAD` and `git clean -fd`).
pub fn reset_hard_clean(worktree: &Path) -> Result<(), FlowError> {
    let reset = Command::new("git")
        .args(["reset", "--hard", "HEAD"])
        .current_dir(worktree)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to run git reset: {err}")))?;

    if !reset.status.success() {
        let err_msg = String::from_utf8_lossy(&reset.stderr);
        return Err(FlowError::Git(format!("git reset failed: {err_msg}")));
    }

    let clean = Command::new("git")
        .args(["clean", "-fd"])
        .current_dir(worktree)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to run git clean: {err}")))?;

    if !clean.status.success() {
        let err_msg = String::from_utf8_lossy(&clean.stderr);
        return Err(FlowError::Git(format!("git clean failed: {err_msg}")));
    }

    Ok(())
}

/// Commits all changes in worktree with author `arqus-flow <flow@localhost>`.
/// Never changes the user's git config.
pub fn commit_changes(
    worktree: &Path,
    task_id: &str,
    task_title: &str,
    executor: &str,
    model: &str,
) -> Result<String, FlowError> {
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(worktree)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to git add: {err}")))?;

    if !add.status.success() {
        let err_msg = String::from_utf8_lossy(&add.stderr);
        return Err(FlowError::Git(format!("git add -A failed: {err_msg}")));
    }

    let title_msg = format!("arqus flow: {task_id} {task_title}");
    let body_msg = format!("executor: {executor}\nmodel: {model}");

    let commit = Command::new("git")
        .args([
            "-c",
            "user.name=arqus-flow",
            "-c",
            "user.email=flow@localhost",
            "commit",
            "-m",
            &title_msg,
            "-m",
            &body_msg,
        ])
        .current_dir(worktree)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to git commit: {err}")))?;

    if !commit.status.success() {
        let err_msg = String::from_utf8_lossy(&commit.stderr);
        return Err(FlowError::Git(format!("git commit failed: {err_msg}")));
    }

    get_head_commit(worktree)
}

/// Returns a sorted list of all tracked file paths in the repository using `git ls-files`.
pub fn list_tracked_files(repo: &Path) -> Result<Vec<String>, FlowError> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(repo)
        .output()
        .map_err(|err| FlowError::Git(format!("failed to run git ls-files: {err}")))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(FlowError::Git(format!("git ls-files failed: {err_msg}")));
    }

    let raw = output.stdout;
    let mut files = Vec::new();
    for entry in raw.split(|&b| b == 0) {
        if !entry.is_empty() {
            let path_str = String::from_utf8_lossy(entry).to_string();
            files.push(path_str);
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
