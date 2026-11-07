//! Tests for the `read` tool.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::*;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pirs_read_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn truncate_bytes_never_splits_a_character() {
    let text = "ééé";
    let cut = truncate_bytes(text, 3);
    assert_eq!(cut, "é");
    assert_eq!(cut.len(), 2);

    let text = "😀😀";
    let cut = truncate_bytes(text, 5);
    assert_eq!(cut, "😀");
    assert_eq!(cut.len(), 4);
}

#[test]
fn nul_byte_means_binary() {
    let dir = temp_dir();
    std::fs::write(dir.join("bin.dat"), [0xff, 0xfe, 0x00, 0x01]).unwrap();
    let out = run(&json!({"path": "bin.dat"}), &dir);
    assert!(out.is_error);
    assert!(out.text.contains("binary file"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn invalid_utf8_without_nul_is_error() {
    let dir = temp_dir();
    std::fs::write(dir.join("latin1.txt"), [0xff, 0xfe]).unwrap();
    let out = run(&json!({"path": "latin1.txt"}), &dir);
    assert!(out.is_error);
    assert!(out.text.contains("UTF-8"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn long_line_is_cut_with_marker() {
    let dir = temp_dir();
    let line = "a".repeat(MAX_LINE_CHARS + 500);
    std::fs::write(dir.join("min.js"), &line).unwrap();
    let out = run(&json!({"path": "min.js"}), &dir);
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("...[line truncated]"), "{}", out.text);
    assert!(out.text.starts_with(&"a".repeat(MAX_LINE_CHARS)));
}

#[test]
fn long_multibyte_line_is_cut_on_a_char_boundary() {
    let dir = temp_dir();
    let line = "é".repeat(MAX_LINE_CHARS + 500);
    std::fs::write(dir.join("wide.txt"), &line).unwrap();
    let out = run(&json!({"path": "wide.txt"}), &dir);
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("...[line truncated]"), "{}", out.text);
    let head: String = out.text.chars().take(MAX_LINE_CHARS).collect();
    assert!(head.chars().all(|c| c == 'é'));
}

#[test]
fn directory_lists_sorted_with_slash_on_folders() {
    let dir = temp_dir();
    std::fs::create_dir(dir.join("zdir")).unwrap();
    std::fs::create_dir(dir.join("adir")).unwrap();
    std::fs::write(dir.join("b.txt"), "b").unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();

    let out = run(&json!({"path": "."}), &dir);
    assert!(!out.is_error, "{}", out.text);
    assert_eq!(out.text, "a.txt\nadir/\nb.txt\nzdir/");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn offset_past_end_reports_line_count() {
    let dir = temp_dir();
    std::fs::write(dir.join("f.txt"), "a\nb\nc\n").unwrap();
    let out = run(&json!({"path": "f.txt", "offset": 9}), &dir);
    assert!(out.is_error);
    assert!(out.text.contains("past the end"), "{}", out.text);
    assert!(out.text.contains("3 lines"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn crlf_lines_are_read() {
    let dir = temp_dir();
    std::fs::write(dir.join("win.txt"), "a\r\nb\r\n").unwrap();
    let out = run(&json!({"path": "win.txt"}), &dir);
    assert!(!out.is_error, "{}", out.text);
    assert_eq!(out.text, "a\nb\n");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn byte_cap_keeps_head_and_says_how_to_continue() {
    let dir = temp_dir();
    let text = "x".repeat(MAX_BYTES + 10);
    std::fs::write(dir.join("big.txt"), &text).unwrap();
    let out = run(&json!({"path": "big.txt"}), &dir);
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("truncated"), "{}", out.text);
    assert!(out.text.len() <= MAX_BYTES + 100);
}
