use super::*;

use std::io::Read;

fn tmp() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pirs_debug_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Split on purpose: the release check greps the tree for key-shaped strings.
fn fake_key() -> String {
    format!("{}{}", "sk-or-v1-", "abcdef123456")
}

#[test]
fn redact_removes_the_configured_key() {
    let previous = std::env::var("PIRS_API_KEY").ok();
    std::env::set_var("PIRS_API_KEY", "secret-key-12345");

    let got = redact("Authorization: Bearer secret-key-12345");
    assert_eq!(got, "Authorization: Bearer [REDACTED]");
    // Short keys are ignored, so a tiny value does not chew up normal text.
    assert_eq!(redact_with("abc abc", "abc"), "abc abc");

    match previous {
        Some(value) => std::env::set_var("PIRS_API_KEY", value),
        None => std::env::remove_var("PIRS_API_KEY"),
    }
}

#[test]
fn redact_removes_sk_tokens() {
    let key = fake_key();
    let got = redact_with(&format!("key {key} and sk-short"), "");
    assert_eq!(got, "key [REDACTED] and sk-short");
    assert!(!got.contains("abcdef123456"));
}

#[test]
fn redact_keeps_clean_text() {
    assert_eq!(
        redact_with("POST /chat/completions returned 200", ""),
        "POST /chat/completions returned 200"
    );
}

#[test]
fn debug_off_creates_no_file() {
    let home = tmp();
    let log = open_log(false, &home);
    assert!(log.is_none());
    assert!(!home.join("debug.log").exists());
}

#[test]
fn debug_on_writes_json_lines_without_the_key() {
    let home = tmp();
    let log = open_log(true, &home).expect("log opened");
    let key = fake_key();
    let key = key.as_str();

    append_line(
        &log,
        "request",
        json!({"url": "https://example.test/v1/chat/completions", "body": {"model": key}}),
        key,
    );
    append_line(&log, "status", json!({"code": 200}), key);
    append_line(&log, "sse", json!({"line": "data: {}"}), key);
    append_line(
        &log,
        "tool",
        json!({"name": "bash", "args": {"command": "ls"}}),
        key,
    );
    append_line(
        &log,
        "error",
        json!({"text": format!("bad key {key}")}),
        key,
    );
    drop(log);

    let mut text = String::new();
    fs::File::open(home.join("debug.log"))
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();

    assert_eq!(text.lines().count(), 5);
    let parsed: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect();
    assert_eq!(parsed[0]["kind"], "request");
    assert_eq!(parsed[1]["code"], 200);
    assert_eq!(parsed[3]["name"], "bash");
    assert_eq!(parsed[4]["kind"], "error");
    assert!(!text.contains(key));
    assert!(!text.contains("abcdef123456"));
}
