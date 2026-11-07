//! Tests for the `write` tool.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::*;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pirs_write_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn reports_byte_count_and_creates_parents() {
    let dir = temp_dir();
    let out = run(&json!({"path": "a/b.txt", "content": "hi"}), &dir);
    assert!(!out.is_error, "{}", out.text);
    assert_eq!(out.text, "Wrote 2 bytes to a/b.txt");
    assert_eq!(std::fs::read_to_string(dir.join("a/b.txt")).unwrap(), "hi");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn directory_target_is_refused() {
    let dir = temp_dir();
    std::fs::create_dir(dir.join("folder")).unwrap();
    let out = run(&json!({"path": "folder", "content": "x"}), &dir);
    assert!(out.is_error);
    assert!(out.text.contains("directory"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[cfg(unix)]
#[test]
fn keeps_permissions_of_an_existing_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir();
    let path = dir.join("script.sh");
    std::fs::write(&path, "old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();

    let out = run(&json!({"path": "script.sh", "content": "new"}), &dir);
    assert!(!out.is_error, "{}", out.text);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "mode was {mode:o}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn leaves_no_temp_file_behind() {
    let dir = temp_dir();
    let out = run(&json!({"path": "a.txt", "content": "one"}), &dir);
    assert!(!out.is_error, "{}", out.text);
    let out = run(&json!({"path": "a.txt", "content": "two"}), &dir);
    assert!(!out.is_error, "{}", out.text);

    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["a.txt".to_string()],
        "temp file left: {names:?}"
    );
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "two");
    std::fs::remove_dir_all(dir).ok();
}
