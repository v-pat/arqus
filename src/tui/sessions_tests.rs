use super::*;
use std::io::Write;
use std::time::Instant;

fn tmp() -> PathBuf {
    std::env::temp_dir().join(format!("arqus_info_{}", crate::session::new_id()))
}

fn write_session(lines: &[String]) -> PathBuf {
    let dir = tmp();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("100_test.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(
        file,
        "{{\"type\":\"session\",\"version\":1,\"id\":\"t\",\"cwd\":\"/x\",\"ts\":0}}"
    )
    .unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
    path
}

fn user(text: &str) -> String {
    format!(
        "{{\"id\":\"a\",\"parent_id\":null,\"ts\":0,\"type\":\"message\",\"message\":{{\"role\":\"user\",\"text\":{text}}}}}"
    )
}

fn rename(name: &str) -> String {
    format!(
        "{{\"id\":\"b\",\"parent_id\":\"a\",\"ts\":0,\"type\":\"custom\",\"custom_type\":\"session_name\",\"data\":{{\"name\":{name}}}}}"
    )
}

#[test]
fn first_user_message_is_the_title() {
    let path = write_session(&[user("\"fix the tests\"")]);
    let info = read(&path).unwrap();
    assert_eq!(info.title, "fix the tests");
    assert_eq!(info.messages, 1);
}

#[test]
fn the_last_rename_wins() {
    let path = write_session(&[
        user("\"hello\""),
        rename("\"first\""),
        rename("\"release prep\""),
    ]);
    let info = read(&path).unwrap();
    assert_eq!(info.title, "release prep");
    assert_eq!(info.messages, 3);
}

#[test]
fn an_empty_session_has_a_readable_title() {
    let path = write_session(&[]);
    let info = read(&path).unwrap();
    assert_eq!(info.title, "(empty session)");
    assert_eq!(info.messages, 0);
}

#[test]
fn a_long_title_is_cut_to_60_characters() {
    let long = "a".repeat(80);
    let path = write_session(&[user(&format!("\"{long}\""))]);
    let info = read(&path).unwrap();
    assert_eq!(info.title.chars().count(), 60);
}

#[test]
fn ten_thousand_lines_are_read_fast() {
    let lines: Vec<String> = (0..10_000)
        .map(|index| user(&format!("\"message {index}\"")))
        .collect();
    let path = write_session(&lines);

    let started = Instant::now();
    let info = read(&path).unwrap();
    let elapsed = started.elapsed();

    assert_eq!(info.messages, 10_000);
    assert_eq!(info.title, "message 0");
    assert!(
        elapsed.as_secs_f64() < 1.0,
        "10k lines took {elapsed:?} in a debug build"
    );
}
