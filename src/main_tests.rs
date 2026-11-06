use crate::compaction::*;
use crate::context::*;
use crate::session::*;
use crate::types::*;
use serde_json::json;

fn user(t: &str) -> EntryKind {
    EntryKind::Message {
        message: Message::User { text: t.into() },
    }
}
fn asst(blocks: Vec<Block>, stop: StopReason) -> EntryKind {
    EntryKind::Message {
        message: Message::Assistant {
            blocks,
            provider: "p".into(),
            model: "m".into(),
            usage: None,
            stop,
        },
    }
}
fn call(id: &str) -> Block {
    Block::ToolCall {
        id: id.into(),
        name: "read".into(),
        args: json!({"path":"a.rs"}),
    }
}
fn result(id: &str) -> EntryKind {
    EntryKind::Message {
        message: Message::ToolResult {
            call_id: id.into(),
            tool_name: "read".into(),
            text: "ok".into(),
            is_error: false,
        },
    }
}
fn tmp() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("pichk_{}", new_id()))
}

#[test]
fn roundtrip_and_reload() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x/y").unwrap();
    s.append(user("hi")).unwrap();
    s.append(asst(
        vec![Block::Text {
            text: "hello".into(),
        }],
        StopReason::Stop,
    ))
    .unwrap();
    let s2 = Session::open(&s.path).unwrap();
    assert_eq!(s2.entries.len(), 2);
    assert_eq!(s2.entries, s.entries);
    assert_eq!(s2.leaf_id, s.leaf_id);
}

#[test]
fn crash_partial_line_is_skipped() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("hi")).unwrap();
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&s.path)
        .unwrap();
    write!(f, "{{\"id\":\"zz\",\"par").unwrap();
    let s2 = Session::open(&s.path).unwrap();
    assert_eq!(s2.entries.len(), 1);
}

#[test]
fn branching_follows_leaf() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    let a = s.append(user("a")).unwrap();
    s.append(user("b")).unwrap();
    s.set_leaf(&a).unwrap();
    s.append(user("c")).unwrap();
    let ctx = build_context(&s.entries, s.leaf_id.as_deref());
    let texts: Vec<_> = ctx
        .iter()
        .map(|m| match m {
            Message::User { text } => text.clone(),
            _ => "?".into(),
        })
        .collect();
    assert_eq!(texts, vec!["a", "c"]);
    // reopen: last line is the leaf, so same result
    let s2 = Session::open(&s.path).unwrap();
    assert_eq!(build_context(&s2.entries, s2.leaf_id.as_deref()).len(), 2);
}

#[test]
fn orphan_tool_call_gets_synthetic_result() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("go")).unwrap();
    s.append(asst(vec![call("c1")], StopReason::ToolUse))
        .unwrap();
    s.append(user("hello?")).unwrap(); // crash happened before result was written
    let ctx = build_context(&s.entries, s.leaf_id.as_deref());
    assert!(matches!(ctx[2], Message::ToolResult { is_error: true, .. }));
    assert!(matches!(ctx[3], Message::User { .. }));
}

#[test]
fn orphan_result_dropped() {
    let ctx = sanitize(vec![
        Message::User { text: "x".into() },
        Message::ToolResult {
            call_id: "nope".into(),
            tool_name: "read".into(),
            text: "".into(),
            is_error: false,
        },
    ]);
    assert_eq!(ctx.len(), 1);
}

#[test]
fn compaction_replaces_old_messages() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("old1")).unwrap();
    s.append(asst(
        vec![Block::Text {
            text: "old2".into(),
        }],
        StopReason::Stop,
    ))
    .unwrap();
    let keep = s.append(user("recent1")).unwrap();
    s.append(asst(
        vec![Block::Text {
            text: "recent2".into(),
        }],
        StopReason::Stop,
    ))
    .unwrap();
    s.append(EntryKind::Compaction {
        summary: "SUMMARY".into(),
        first_kept_id: keep,
        tokens_before: 1000,
        read_files: vec!["a.rs".into()],
        modified_files: vec![],
    })
    .unwrap();
    s.append(user("after")).unwrap();
    let ctx = build_context(&s.entries, s.leaf_id.as_deref());
    assert_eq!(ctx.len(), 4); // summary, recent1, recent2, after
    match &ctx[0] {
        Message::User { text } => {
            assert!(text.contains("SUMMARY"));
            assert!(text.contains("<read-files>"));
        }
        _ => panic!(),
    }
}

#[test]
fn cut_point_never_on_tool_result() {
    let dir = tmp();
    let mut s = Session::create(&dir, "/x").unwrap();
    s.append(user("first question")).unwrap();
    s.append(asst(vec![call("c1")], StopReason::ToolUse))
        .unwrap();
    s.append(EntryKind::Message {
        message: Message::ToolResult {
            call_id: "c1".into(),
            tool_name: "read".into(),
            text: "x".repeat(400),
            is_error: false,
        },
    })
    .unwrap();
    s.append(user("short")).unwrap();
    let path = path_to_root(&s.entries, s.leaf_id.as_deref());
    // the big tool result (100 tokens) pushes us over the budget, but we may not cut there:
    // the cut must move back to the assistant message that made the call (index 1).
    let c = find_cut_point(&path, 50).unwrap();
    assert_eq!(c, 1);
    assert!(!matches!(
        &path[c].kind,
        EntryKind::Message {
            message: Message::ToolResult { .. }
        }
    ));
}

#[test]
fn threshold() {
    assert!(should_compact(185_000, 200_000, 16_384));
    assert!(!should_compact(150_000, 200_000, 16_384));
}

#[test]
fn slug() {
    assert_eq!(project_slug("/home/me/my.proj"), "-home-me-my-proj");
}
