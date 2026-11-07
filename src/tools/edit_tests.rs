//! Tests for the `edit` tool.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::*;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_file(name: &str, content: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pirs_edit_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn zero_matches_is_error_with_line_count_and_hint() {
    let path = temp_file("zero.txt", "hello\nworld\n");
    let dir = path.parent().unwrap().to_path_buf();
    let out = run(
        &json!({"path": "zero.txt", "old_text": "nope", "new_text": "x"}),
        &dir,
    );
    assert!(out.is_error);
    assert!(out.text.contains("not found"), "{}", out.text);
    assert!(out.text.contains("2 lines"), "{}", out.text);
    assert!(out.text.contains("tabs"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn two_matches_is_error() {
    let path = temp_file("two.txt", "a\na\n");
    let dir = path.parent().unwrap().to_path_buf();
    let out = run(
        &json!({"path": "two.txt", "old_text": "a", "new_text": "b"}),
        &dir,
    );
    assert!(out.is_error);
    assert!(out.text.contains("2 times"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn one_match_edits_file() {
    let path = temp_file("one.txt", "a\nb\nc\n");
    let dir = path.parent().unwrap().to_path_buf();
    let out = run(
        &json!({"path": "one.txt", "old_text": "b", "new_text": "B"}),
        &dir,
    );
    assert!(!out.is_error, "{}", out.text);
    assert_eq!(out.text, "Edited one.txt");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nB\nc\n");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn empty_old_text_is_error() {
    let path = temp_file("empty.txt", "abc\n");
    let dir = path.parent().unwrap().to_path_buf();
    let out = run(
        &json!({"path": "empty.txt", "old_text": "", "new_text": "x"}),
        &dir,
    );
    assert!(out.is_error);
    assert!(out.text.contains("must not be empty"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn identical_text_is_error() {
    let path = temp_file("same.txt", "abc\n");
    let dir = path.parent().unwrap().to_path_buf();
    let out = run(
        &json!({"path": "same.txt", "old_text": "abc", "new_text": "abc"}),
        &dir,
    );
    assert!(out.is_error);
    assert!(out.text.contains("nothing to edit"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn lf_old_text_matches_crlf_file_and_keeps_crlf() {
    let path = temp_file("win.txt", "a\r\nb\r\nc\r\n");
    let dir = path.parent().unwrap().to_path_buf();
    let out = run(
        &json!({"path": "win.txt", "old_text": "a\nb", "new_text": "a\nB"}),
        &dir,
    );
    assert!(!out.is_error, "{}", out.text);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\r\nB\r\nc\r\n");
    std::fs::remove_dir_all(dir).ok();
}

#[cfg(unix)]
#[test]
fn keeps_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let path = temp_file("mode.txt", "abc\n");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let dir = path.parent().unwrap().to_path_buf();
    let out = run(
        &json!({"path": "mode.txt", "old_text": "abc", "new_text": "xyz"}),
        &dir,
    );
    assert!(!out.is_error, "{}", out.text);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640, "mode was {mode:o}");
    std::fs::remove_dir_all(dir).ok();
}
