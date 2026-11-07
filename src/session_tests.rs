use super::*;
use crate::context::build_context;
use std::io::Write;
use std::time::Instant;

fn tmp() -> PathBuf {
    std::env::temp_dir().join(format!("pirs_session_{}", new_id()))
}

fn user(t: &str) -> EntryKind {
    EntryKind::Message {
        message: Message::User {
            text: t.to_string(),
        },
    }
}

fn texts(entries: &[Entry], leaf: Option<&str>) -> Vec<String> {
    path_to_root(entries, leaf)
        .into_iter()
        .map(|e| match &e.kind {
            EntryKind::Message {
                message: Message::User { text },
            } => text.clone(),
            _ => "?".to_string(),
        })
        .collect()
}

#[test]
fn reopen_keeps_entries_and_leaf() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("a")).unwrap();
    s.append(user("b")).unwrap();
    let s2 = Session::open(&s.path).unwrap();
    assert_eq!(s2.entries, s.entries);
    assert_eq!(s2.leaf_id, s.leaf_id);
}

#[test]
fn broken_last_line_is_skipped() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("a")).unwrap();
    let mut f = OpenOptions::new().append(true).open(&s.path).unwrap();
    write!(f, "{{\"id\":\"zz\",\"par").unwrap();
    let s2 = Session::open(&s.path).unwrap();
    assert_eq!(s2.entries.len(), 1);
}

#[test]
fn corrupt_middle_line_is_skipped_and_the_rest_still_loads() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("a")).unwrap();
    s.append(user("b")).unwrap();
    let text = fs::read_to_string(&s.path).unwrap();
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    lines.insert(2, "{not json at all".to_string());
    fs::write(&s.path, format!("{}\n", lines.join("\n"))).unwrap();

    let s2 = Session::open(&s.path).unwrap();
    assert_eq!(s2.entries.len(), 2, "good lines around the bad one survive");
    assert_eq!(texts(&s2.entries, s2.leaf_id.as_deref()), vec!["a", "b"]);
    assert_eq!(build_context(&s2.entries, s2.leaf_id.as_deref()).len(), 2);
}

#[test]
fn branch_moves_leaf() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    let a = s.append(user("a")).unwrap();
    s.append(user("b")).unwrap();
    s.set_leaf(&a).unwrap();
    s.append(user("c")).unwrap();
    assert_eq!(texts(&s.entries, s.leaf_id.as_deref()), vec!["a", "c"]);
}

#[test]
fn reopened_leaf_is_last_line() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("a")).unwrap();
    let b = s.append(user("b")).unwrap();
    let s2 = Session::open(&s.path).unwrap();
    assert_eq!(s2.leaf_id.as_deref(), Some(b.as_str()));
}

#[test]
fn slug_replaces_slash_and_dot() {
    assert_eq!(project_slug("/home/me/my.proj"), "-home-me-my-proj");
}

#[test]
fn latest_session_picks_newest() {
    let dir = tmp();
    fs::create_dir_all(&dir).unwrap();
    let old = dir.join("100_a.jsonl");
    let new = dir.join("200_b.jsonl");
    fs::write(&old, "").unwrap();
    fs::write(&new, "").unwrap();
    assert_eq!(latest_session(&dir), Some(new));
}

#[test]
fn list_sessions_is_newest_first() {
    let dir = tmp();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("100_a.jsonl"), "").unwrap();
    fs::write(dir.join("300_c.jsonl"), "").unwrap();
    fs::write(dir.join("200_b.jsonl"), "").unwrap();
    fs::write(dir.join("notes.txt"), "").unwrap();
    let names: Vec<String> = list_sessions(&dir)
        .into_iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();
    assert_eq!(names, vec!["300_c.jsonl", "200_b.jsonl", "100_a.jsonl"]);
}

#[test]
fn sessions_dir_ends_with_slug() {
    let p = sessions_dir("/home/me/my.proj");
    assert!(p.ends_with("sessions/-home-me-my-proj"));
}

#[test]
fn twenty_thousand_entries_open_and_build_in_under_two_seconds() {
    let dir = tmp();
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("20000.jsonl");
    let mut text = String::from(
        "{\"type\":\"session\",\"version\":1,\"id\":\"perf\",\"cwd\":\"/x\",\"ts\":0}\n",
    );
    for i in 0..20_000u32 {
        let parent = if i == 0 {
            "null".to_string()
        } else {
            format!("\"e{}\"", i - 1)
        };
        text.push_str(&format!(
            "{{\"id\":\"e{i}\",\"parent_id\":{parent},\"ts\":{i},\"type\":\"message\",\"message\":{{\"role\":\"user\",\"text\":\"m{i}\"}}}}\n"
        ));
    }
    fs::write(&path, text).unwrap();

    let started = Instant::now();
    let session = Session::open(&path).unwrap();
    let context = build_context(&session.entries, session.leaf_id.as_deref());
    let elapsed = started.elapsed();

    assert_eq!(session.entries.len(), 20_000);
    assert_eq!(context.len(), 20_000);
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "20k entries took {elapsed:?} in a debug build"
    );
}
