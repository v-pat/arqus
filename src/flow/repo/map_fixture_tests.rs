use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use super::*;

fn run_git(cwd: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_git_repo() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let repo = dir.path().to_path_buf();

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.name", "Test User"]);
    run_git(&repo, &["config", "user.email", "test@example.com"]);

    (dir, repo)
}

#[test]
fn test_tracked_files_from_git_ls_files_only() {
    let (_dir, repo) = init_git_repo();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/lib.rs"), "pub fn tracked() {}\n").unwrap();
    std::fs::write(repo.join("untracked.rs"), "pub fn untracked() {}\n").unwrap();

    run_git(&repo, &["add", "src/lib.rs"]);
    run_git(&repo, &["commit", "-m", "add tracked"]);

    let map = build_repo_map(&repo, 10_000).expect("build repo map");
    assert!(map.contains("src/lib.rs"));
    assert!(map.contains("pub fn tracked()"));
    assert!(!map.contains("untracked.rs"));
    assert!(!map.contains("untracked()"));
}

#[test]
fn test_symbol_scanners_on_fixture_files() {
    let (_dir, repo) = init_git_repo();

    std::fs::write(
        repo.join("lib.rs"),
        "pub struct AppConfig;\npub fn run_app() {}\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("helper.py"),
        "class Helper:\n    def help(self):\n        pass\n",
    )
    .unwrap();
    std::fs::write(repo.join("util.go"), "package util\nfunc DoUtil() {}\n").unwrap();
    std::fs::write(
        repo.join("client.ts"),
        "export interface Client {}\nexport const CLIENT_VERSION = '1.0';\n",
    )
    .unwrap();

    run_git(
        &repo,
        &["add", "lib.rs", "helper.py", "util.go", "client.ts"],
    );
    run_git(&repo, &["commit", "-m", "add fixtures"]);

    let map = build_repo_map(&repo, 20_000).expect("build repo map");
    assert!(map.contains("lib.rs"));
    assert!(map.contains("pub struct AppConfig"));
    assert!(map.contains("pub fn run_app()"));

    assert!(map.contains("helper.py"));
    assert!(map.contains("class Helper:"));
    assert!(map.contains("def help(self):"));

    assert!(map.contains("util.go"));
    assert!(map.contains("func DoUtil()"));

    assert!(map.contains("client.ts"));
    assert!(map.contains("export interface Client"));
    assert!(map.contains("export const CLIENT_VERSION"));
}

#[test]
fn test_repo_map_size_cap_with_truncation_marker() {
    let (_dir, repo) = init_git_repo();

    // Create several files with multiple symbols
    for i in 0..10 {
        let name = format!("file_{i}.rs");
        let content = format!(
            "pub fn function_{i}_alpha() {{}}\npub struct Struct_{i}_Beta;\npub enum Enum_{i}_Gamma {{}}\n"
        );
        std::fs::write(repo.join(&name), content).unwrap();
        run_git(&repo, &["add", &name]);
    }
    run_git(&repo, &["commit", "-m", "add many files"]);

    let full_map = build_repo_map(&repo, 50_000).expect("full map");
    assert!(full_map.len() > 300);

    let max_bytes = 250;
    let capped_map = build_repo_map(&repo, max_bytes).expect("capped map");
    assert!(
        capped_map.len() <= max_bytes,
        "capped map len {} must be <= max_bytes {}",
        capped_map.len(),
        max_bytes
    );
    assert!(capped_map.contains(TRUNCATION_MARKER.trim()));
}

#[test]
fn test_repo_map_stable_output_byte_deterministic() {
    let (_dir, repo) = init_git_repo();

    std::fs::write(repo.join("b.rs"), "pub fn b_fn() {}\n").unwrap();
    std::fs::write(repo.join("a.rs"), "pub fn a_fn() {}\n").unwrap();
    std::fs::write(repo.join("c.rs"), "pub fn c_fn() {}\n").unwrap();

    run_git(&repo, &["add", "a.rs", "b.rs", "c.rs"]);
    run_git(&repo, &["commit", "-m", "add files"]);

    let map1 = build_repo_map(&repo, 10_000).expect("map 1");
    let map2 = build_repo_map(&repo, 10_000).expect("map 2");

    assert_eq!(map1, map2);
    assert_eq!(map1.as_bytes(), map2.as_bytes());

    // a.rs should appear before b.rs, which appears before c.rs (sorted)
    let pos_a = map1.find("a.rs").expect("a.rs present");
    let pos_b = map1.find("b.rs").expect("b.rs present");
    let pos_c = map1.find("c.rs").expect("c.rs present");
    assert!(
        pos_a < pos_b && pos_b < pos_c,
        "files must be lexicographically sorted"
    );
}

