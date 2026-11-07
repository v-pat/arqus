use super::*;
use std::path::PathBuf;

use uuid::Uuid;

fn temp_path() -> PathBuf {
    std::env::temp_dir().join(format!("pirs-history-{}.jsonl", Uuid::new_v4()))
}

#[test]
fn missing_file_starts_empty() {
    let history = History::load(temp_path());
    assert!(history.is_empty());
    assert_eq!(history.len(), 0);
}

#[test]
fn remember_saves_and_load_reads_back() {
    let path = temp_path();
    let mut history = History::load(path.clone());
    assert!(history.remember("first"));
    assert!(history.remember("second line\nwith newline"));

    let loaded = History::load(path.clone());
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded.get(0), Some("first"));
    assert_eq!(loaded.get(1), Some("second line\nwith newline"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn consecutive_duplicates_and_empty_text_are_skipped() {
    let mut history = History::new();
    assert!(!history.remember("  "));
    assert!(history.remember("same"));
    assert!(!history.remember("same"));
    assert!(history.remember("other"));
    assert!(history.remember("same"));
    assert_eq!(history.len(), 3);
}

#[test]
fn load_ignores_bad_lines_and_keeps_the_newest_500() {
    let path = temp_path();
    std::fs::write(&path, "\"a\"\nnot json\n\"b\"\n\n\"c\"\n").unwrap();
    let history = History::load(path.clone());
    assert_eq!(history.len(), 3);
    assert_eq!(history.get(2), Some("c"));

    let mut big: Vec<String> = (0..505).map(|i| i.to_string()).collect();
    trim(&mut big);
    assert_eq!(big.len(), MAX_ENTRIES);
    assert_eq!(big.first().map(String::as_str), Some("5"));
    assert_eq!(big.last().map(String::as_str), Some("504"));
    let _ = std::fs::remove_file(path);
}
