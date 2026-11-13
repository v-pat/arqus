use super::*;
use crate::session::new_id;
use crate::types::{Message, StopReason};

fn tmp() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("arqus_tree_{}", new_id()))
}

fn user(text: &str) -> EntryKind {
    EntryKind::Message {
        message: Message::User {
            text: text.to_string(),
        },
    }
}

fn session_with_branch() -> Session {
    let dir = tmp();
    let mut session = Session::create(&dir, "/x").unwrap();
    session.append(user("root")).unwrap();
    let a = session.entries[0].id.clone();
    session.append(user("main branch")).unwrap();
    session.set_leaf(&a).unwrap();
    session.append(user("side branch")).unwrap();
    session
}

#[test]
fn rows_show_depth_and_path() {
    let session = session_with_branch();
    let rows = rows(&session.entries, session.leaf_id.as_deref());
    assert_eq!(rows.len(), 3);

    assert_eq!(rows[0].depth, 0);
    assert_eq!(rows[0].role, "user");
    assert_eq!(rows[0].preview, "root");
    assert!(rows[0].on_path);

    // "main branch" is a sibling of the leaf path, so depth 1 but not on it.
    assert_eq!(rows[1].depth, 1);
    assert!(!rows[1].on_path);

    assert_eq!(rows[2].depth, 1);
    assert!(rows[2].on_path);
    assert_eq!(rows[2].preview, "side branch");
}

#[test]
fn preview_is_cut_to_60_characters_and_flattens_lines() {
    let text = format!("{}\nsecond line", "a".repeat(80));
    let shown = preview(&text);
    assert_eq!(shown.chars().count(), 60);
    assert!(!shown.contains('\n'));
}

#[test]
fn roles_cover_tool_and_non_message_entries() {
    let entries = vec![
        Entry {
            id: "a".to_string(),
            parent_id: None,
            ts: 0,
            kind: EntryKind::Message {
                message: Message::Assistant {
                    blocks: vec![Block::ToolCall {
                        id: "c1".to_string(),
                        name: "bash".to_string(),
                        args: serde_json::json!({"command": "ls"}),
                    }],
                    provider: "p".to_string(),
                    model: "m".to_string(),
                    usage: None,
                    stop: StopReason::ToolUse,
                },
            },
        },
        Entry {
            id: "b".to_string(),
            parent_id: Some("a".to_string()),
            ts: 0,
            kind: EntryKind::Message {
                message: Message::ToolResult {
                    call_id: "c1".to_string(),
                    tool_name: "bash".to_string(),
                    text: "ok".to_string(),
                    is_error: false,
                },
            },
        },
        Entry {
            id: "c".to_string(),
            parent_id: Some("b".to_string()),
            ts: 0,
            kind: EntryKind::ModelChange {
                provider: "anthropic".to_string(),
                model: "claude".to_string(),
            },
        },
    ];
    let rows = rows(&entries, Some("c"));
    assert_eq!(rows[0].role, "assistant");
    assert_eq!(rows[0].preview, "tool_call: bash");
    assert_eq!(rows[1].role, "tool_result");
    assert_eq!(rows[2].role, "model_change");
    assert_eq!(rows[2].preview, "anthropic:claude");
    assert_eq!(rows[2].depth, 2);
}

#[test]
fn fork_keeps_the_path_and_drops_other_branches() {
    let dir = tmp();
    let session = session_with_branch();
    let old_ids: Vec<String> = path_to_root(&session.entries, session.leaf_id.as_deref())
        .into_iter()
        .map(|e| e.id.clone())
        .collect();
    assert_eq!(old_ids.len(), 2);

    let forked = fork(&session, &dir).unwrap();
    let new_ids: Vec<String> = forked.entries.iter().map(|e| e.id.clone()).collect();
    assert_eq!(new_ids, old_ids);
    assert_eq!(forked.leaf_id, session.leaf_id);
    assert_ne!(forked.header.id, session.header.id);
    assert_ne!(forked.path, session.path);

    let texts: Vec<String> = forked
        .entries
        .iter()
        .map(|e| match &e.kind {
            EntryKind::Message {
                message: Message::User { text },
            } => text.clone(),
            other => panic!("expected user message, got {other:?}"),
        })
        .collect();
    assert_eq!(texts, vec!["root", "side branch"]);

    // The copied file really exists and reopens to the same path.
    let reopened = Session::open(&forked.path).unwrap();
    assert_eq!(reopened.leaf_id, session.leaf_id);
    assert_eq!(reopened.entries.len(), 2);
}
