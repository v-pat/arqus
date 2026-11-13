//! Integration tests for `tools::execute` and `tools::tool_defs`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use serde_json::json;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "arqus_tools_{}_{}_{}",
        std::process::id(),
        nanos,
        n
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn temp_dirs_are_unique() {
    assert_ne!(temp_dir(), temp_dir());
}

#[test]
fn four_definitions_with_schemas() {
    let defs = tool_defs();
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["read", "write", "edit", "bash"]);
    for def in defs {
        assert_eq!(def.parameters["type"], "object");
        assert!(def.parameters["required"].is_array());
    }
}

#[tokio::test]
async fn unknown_tool_is_error() {
    let dir = temp_dir();
    let out = execute("grep", &json!({}), &dir).await;
    assert!(out.is_error);
    assert!(out.text.contains("unknown tool"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn missing_argument_is_error() {
    let dir = temp_dir();
    let out = execute("read", &json!({}), &dir).await;
    assert!(out.is_error);
    assert!(out.text.contains("path"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn write_then_read_back() {
    let dir = temp_dir();
    let written = execute(
        "write",
        &json!({"path": "sub/a.txt", "content": "alpha\nbeta\n"}),
        &dir,
    )
    .await;
    assert!(!written.is_error, "{}", written.text);
    assert!(written.text.contains("Wrote 11 bytes"), "{}", written.text);

    let read = execute("read", &json!({"path": "sub/a.txt"}), &dir).await;
    assert!(!read.is_error, "{}", read.text);
    assert_eq!(read.text, "alpha\nbeta\n");
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn read_window() {
    let dir = temp_dir();
    std::fs::write(dir.join("f.txt"), "1\n2\n3\n4\n5\n").unwrap();
    let out = execute(
        "read",
        &json!({"path": "f.txt", "offset": 2, "limit": 2}),
        &dir,
    )
    .await;
    assert!(!out.is_error, "{}", out.text);
    assert_eq!(
        out.text,
        "2\n3\n[truncated: call read again with offset=4 to continue]"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn read_missing_file_is_error() {
    let dir = temp_dir();
    let out = execute("read", &json!({"path": "nope.txt"}), &dir).await;
    assert!(out.is_error);
    assert!(out.text.contains("could not read"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn read_binary_is_error() {
    let dir = temp_dir();
    std::fs::write(dir.join("bad.bin"), [0xff, 0xfe, 0x00]).unwrap();
    let out = execute("read", &json!({"path": "bad.bin"}), &dir).await;
    assert!(out.is_error);
    assert!(out.text.contains("binary file"), "{}", out.text);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn bash_via_dispatch() {
    let dir = temp_dir();
    let out = execute("bash", &json!({"command": "echo hi"}), &dir).await;
    assert!(!out.is_error, "{}", out.text);
    assert!(out.text.contains("hi"));
    assert!(out.text.contains("exit code: 0"));
    std::fs::remove_dir_all(dir).ok();
}
