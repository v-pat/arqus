use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use super::*;

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git command failed");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_test_worktree() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path().to_path_buf();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.name", "Test User"]);
    run_git(&repo, &["config", "user.email", "test@example.com"]);

    std::fs::write(repo.join("file_a.txt"), "line1\nline2\n").unwrap();
    std::fs::write(repo.join("file_b.txt"), "hello\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "Initial commit"]);

    (dir, repo)
}

#[test]
fn test_scope_check_allowed_paths_pass() {
    let (_dir, repo) = init_test_worktree();
    std::fs::write(repo.join("file_a.txt"), "line1\nmodified\n").unwrap();

    let allowed = vec!["file_a.txt".to_string()];
    let res = check_scope(&repo, &allowed, 400);
    assert!(res.is_ok(), "allowed writes should pass: {res:?}");
}

#[test]
fn test_scope_check_extra_untracked_file_fails() {
    let (_dir, repo) = init_test_worktree();
    std::fs::write(repo.join("file_a.txt"), "modified\n").unwrap();
    std::fs::write(repo.join("extra.txt"), "untracked extra\n").unwrap();

    let allowed = vec!["file_a.txt".to_string()];
    let res = check_scope(&repo, &allowed, 400);
    assert!(
        res.is_err(),
        "extra untracked file outside allowed_writes must fail scope check"
    );
}

#[test]
fn test_scope_check_deleted_file_fails() {
    let (_dir, repo) = init_test_worktree();
    std::fs::remove_file(repo.join("file_b.txt")).unwrap();

    let allowed = vec!["file_a.txt".to_string()];
    let res = check_scope(&repo, &allowed, 400);
    assert!(
        res.is_err(),
        "deleted file not in allowed_writes must fail scope check"
    );
}

#[test]
fn test_scope_check_renamed_file_fails() {
    let (_dir, repo) = init_test_worktree();
    // Rename file_b.txt to file_c.txt
    std::fs::rename(repo.join("file_b.txt"), repo.join("file_c.txt")).unwrap();

    let allowed = vec!["file_a.txt".to_string()];
    let res = check_scope(&repo, &allowed, 400);
    assert!(
        res.is_err(),
        "renamed file not in allowed_writes must fail scope check"
    );
}

#[test]
fn test_scope_check_file_outside_repo_fails() {
    let (_dir, repo) = init_test_worktree();
    let allowed = vec!["../outside.txt".to_string()];
    let res = check_scope(&repo, &allowed, 400);
    assert!(res.is_err(), "path outside repo must fail scope check");
}

#[test]
fn test_scope_check_diff_over_max_changed_lines_fails() {
    let (_dir, repo) = init_test_worktree();
    let big_content: String = (0..500).map(|i| format!("line {i}\n")).collect();
    std::fs::write(repo.join("file_a.txt"), big_content).unwrap();

    let allowed = vec!["file_a.txt".to_string()];
    let res = check_scope(&repo, &allowed, 400);
    assert!(
        res.is_err(),
        "diff exceeding max_changed_lines_hard (500 > 400) must fail scope check"
    );
}

#[tokio::test]
async fn test_run_verify_commands_success_and_failure() {
    let (_dir, repo) = init_test_worktree();
    let allowed_commands = vec!["true".to_string(), "false".to_string()];
    let passthrough = vec!["PATH".to_string()];

    // Success
    let cmds_ok = vec!["true".to_string()];
    let res_ok = run_verify_commands(&repo, &cmds_ok, &allowed_commands, &passthrough, 10)
        .await
        .unwrap();
    assert!(res_ok.passed);
    assert_eq!(res_ok.failed_command, None);

    // Failure
    let cmds_fail = vec!["true".to_string(), "false".to_string()];
    let res_fail = run_verify_commands(&repo, &cmds_fail, &allowed_commands, &passthrough, 10)
        .await
        .unwrap();
    assert!(!res_fail.passed);
    assert_eq!(res_fail.failed_command, Some("false".to_string()));
}
