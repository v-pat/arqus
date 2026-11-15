use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use super::*;

fn run_git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git command failed to run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_git_repo() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let repo = dir.path().to_path_buf();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.name", "Test User"]);
    run_git(&repo, &["config", "user.email", "test@example.com"]);

    let readme = repo.join("README.md");
    std::fs::write(&readme, "# Test Repo\n").expect("write readme");
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "-m", "Initial commit"]);

    (dir, repo)
}

#[test]
fn test_clean_repo_accepted() {
    let (_dir, repo) = init_git_repo();
    let res = check_clean_repo(&repo);
    assert!(res.is_ok(), "clean repo should be accepted: {res:?}");
}

#[test]
fn test_dirty_repo_refused_modified_file() {
    let (_dir, repo) = init_git_repo();
    std::fs::write(repo.join("README.md"), "modified content\n").unwrap();
    let res = check_clean_repo(&repo);
    assert!(
        res.is_err(),
        "repo with modified file must be refused as dirty"
    );
}

#[test]
fn test_dirty_repo_refused_untracked_file() {
    let (_dir, repo) = init_git_repo();
    std::fs::write(repo.join("untracked.txt"), "new file\n").unwrap();
    let res = check_clean_repo(&repo);
    assert!(
        res.is_err(),
        "repo with untracked file must be refused as dirty"
    );
}

#[test]
fn test_branch_and_worktree_created() {
    let (_dir, repo) = init_git_repo();
    let run_id = "run123";
    let (branch, worktree) =
        create_branch_and_worktree(&repo, run_id, None).expect("failed to create worktree");

    assert_eq!(branch, format!("arqus/flow/{run_id}"));
    assert!(worktree.exists(), "worktree dir should exist on disk");
    assert!(worktree.join("README.md").exists());

    // Clean up worktree
    run_git(
        &repo,
        &["worktree", "remove", "--force", worktree.to_str().unwrap()],
    );
}

#[test]
fn test_exclude_entry_added_and_gitignore_untouched() {
    let (_dir, repo) = init_git_repo();
    let gitignore = repo.join(".gitignore");
    assert!(!gitignore.exists());

    ensure_exclude_entry(&repo).expect("ensure exclude entry failed");

    let exclude_path = repo.join(".git").join("info").join("exclude");
    assert!(exclude_path.exists());
    let exclude_content = std::fs::read_to_string(&exclude_path).unwrap();
    assert!(exclude_content.contains(".arqus/flow/"));
    assert!(!gitignore.exists(), ".gitignore must remain untouched");

    // Calling twice should not duplicate the entry
    ensure_exclude_entry(&repo).expect("ensure exclude entry idempotent");
    let exclude_content2 = std::fs::read_to_string(&exclude_path).unwrap();
    assert_eq!(
        exclude_content2.matches(".arqus/flow/").count(),
        1,
        "entry should not be duplicated"
    );
}

#[test]
fn test_reset_hard_clean_removes_modified_deleted_and_untracked() {
    let (_dir, repo) = init_git_repo();
    let (_branch, worktree) = create_branch_and_worktree(&repo, "run_clean", None).unwrap();

    // Modify a file
    std::fs::write(worktree.join("README.md"), "changed\n").unwrap();
    // Add an untracked file
    std::fs::write(worktree.join("temp.txt"), "temp\n").unwrap();

    reset_hard_clean(&worktree).expect("reset hard clean failed");

    assert_eq!(
        std::fs::read_to_string(worktree.join("README.md")).unwrap(),
        "# Test Repo\n"
    );
    assert!(!worktree.join("temp.txt").exists());

    run_git(
        &repo,
        &["worktree", "remove", "--force", worktree.to_str().unwrap()],
    );
}

#[test]
fn test_commit_author_is_arqus_flow_and_user_git_config_unchanged() {
    let (_dir, repo) = init_git_repo();
    let (_branch, worktree) = create_branch_and_worktree(&repo, "run_author", None).unwrap();

    std::fs::write(worktree.join("new_file.txt"), "content\n").unwrap();
    let commit_hash = commit_changes(&worktree, "T01", "Add new file", "worker", "qwen3.5:9b")
        .expect("commit failed");

    assert!(!commit_hash.is_empty());

    let author_name = run_git(&worktree, &["log", "-1", "--format=%an"]);
    let author_email = run_git(&worktree, &["log", "-1", "--format=%ae"]);
    let commit_msg = run_git(&worktree, &["log", "-1", "--format=%B"]);

    assert_eq!(author_name, "arqus-flow");
    assert_eq!(author_email, "flow@localhost");
    assert!(commit_msg.contains("arqus flow: T01 Add new file"));
    assert!(commit_msg.contains("executor: worker"));
    assert!(commit_msg.contains("model: qwen3.5:9b"));

    // User's configured git config in the repo should remain "Test User"
    let repo_config_user = run_git(&repo, &["config", "user.name"]);
    assert_eq!(repo_config_user, "Test User");

    run_git(
        &repo,
        &["worktree", "remove", "--force", worktree.to_str().unwrap()],
    );
}

#[test]
fn test_base_branch_head_unchanged_after_worktree_commit() {
    let (_dir, repo) = init_git_repo();
    let base_head = get_head_commit(&repo).unwrap();

    let (_branch, worktree) = create_branch_and_worktree(&repo, "run_head", None).unwrap();

    std::fs::write(worktree.join("file.txt"), "hello\n").unwrap();
    commit_changes(&worktree, "T02", "Add file", "orchestrator", "claude-3-7").unwrap();

    let base_head_after = get_head_commit(&repo).unwrap();
    assert_eq!(
        base_head, base_head_after,
        "base branch HEAD must be unchanged after worktree commit"
    );

    run_git(
        &repo,
        &["worktree", "remove", "--force", worktree.to_str().unwrap()],
    );
}

#[test]
fn test_list_tracked_files() {
    let (_dir, repo) = init_git_repo();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(repo.join("untracked.txt"), "ignored\n").unwrap();
    run_git(&repo, &["add", "src/main.rs"]);

    let tracked = list_tracked_files(&repo).unwrap();
    assert_eq!(tracked, vec!["README.md", "src/main.rs"]);
    assert!(!tracked.contains(&"untracked.txt".to_string()));
}
