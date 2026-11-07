use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::*;
use crate::agent::{run_prompt, AgentConfig};
use crate::ai::StreamEvent;
use crate::context::build_context;
use crate::session::new_id;
use crate::types::{Block, EntryKind, Message, StopReason};

fn tmp() -> PathBuf {
    std::env::temp_dir().join(format!("pirs_comp_{}", new_id()))
}

fn user(text: &str) -> EntryKind {
    EntryKind::Message {
        message: Message::User {
            text: text.to_string(),
        },
    }
}

fn call(id: &str, name: &str, path: &str) -> Block {
    Block::ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args: json!({ "path": path }),
    }
}

fn assistant(blocks: Vec<Block>) -> EntryKind {
    EntryKind::Message {
        message: Message::Assistant {
            blocks,
            provider: "mock".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::Stop,
        },
    }
}

fn summary_events(text: &str) -> Vec<StreamEvent> {
    vec![StreamEvent::Done(Message::Assistant {
        blocks: vec![Block::Text {
            text: text.to_string(),
        }],
        provider: "mock".to_string(),
        model: "m".to_string(),
        usage: None,
        stop: StopReason::Stop,
    })]
}

fn mock(script: Vec<Vec<StreamEvent>>) -> Llm {
    Llm::Mock(Arc::new(Mutex::new(VecDeque::from(script))))
}

fn mock_capture(script: Vec<Vec<StreamEvent>>) -> (Llm, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let llm = Llm::MockCapture(Arc::new(Mutex::new(VecDeque::from(script))), seen.clone());
    (llm, seen)
}

fn settings() -> CompactionSettings {
    CompactionSettings {
        enabled: true,
        reserve_tokens: 0,
        keep_recent_tokens: 100,
    }
}

fn long_history(session: &mut Session, count: usize) {
    for i in 0..count {
        let text = format!("message {i} {}", "x".repeat(100));
        session.append(user(&text)).unwrap();
    }
}

fn last_compaction(session: &Session) -> Option<&EntryKind> {
    session
        .entries
        .last()
        .map(|e| &e.kind)
        .filter(|k| matches!(k, EntryKind::Compaction { .. }))
}

#[test]
fn serialize_conversation_labels_and_cuts_results() {
    let messages = vec![
        Message::User {
            text: "hi".to_string(),
        },
        Message::Assistant {
            blocks: vec![
                Block::Thinking {
                    text: "think".to_string(),
                },
                Block::Text {
                    text: "answer".to_string(),
                },
                call("c1", "read", "src/a.rs"),
            ],
            provider: "p".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::ToolUse,
        },
        Message::ToolResult {
            call_id: "c1".to_string(),
            tool_name: "read".to_string(),
            text: "y".repeat(2500),
            is_error: false,
        },
    ];
    let text = serialize_conversation(&messages);
    assert!(text.contains("[User]: hi"));
    assert!(text.contains("[Assistant thinking]: think"));
    assert!(text.contains("[Assistant]: answer"));
    assert!(text.contains("[Assistant tool calls]: read(path=\"src/a.rs\")"));
    assert!(text.contains("[Tool result]: yyy"));
    let result_line = text
        .lines()
        .find(|l| l.starts_with("[Tool result]: "))
        .unwrap();
    assert_eq!(result_line.chars().count(), "[Tool result]: ".len() + 2000);
}

#[test]
fn extract_file_ops_merges_and_sorts() {
    let entries = vec![
        Entry {
            id: "e1".to_string(),
            parent_id: None,
            ts: 0,
            kind: EntryKind::Compaction {
                summary: "old".to_string(),
                first_kept_id: "e2".to_string(),
                tokens_before: 0,
                read_files: vec!["old.rs".to_string(), "b.rs".to_string()],
                modified_files: vec!["b.rs".to_string()],
                summary_usage: None,
            },
        },
        Entry {
            id: "e2".to_string(),
            parent_id: Some("e1".to_string()),
            ts: 0,
            kind: assistant(vec![
                call("c1", "read", "a.rs"),
                call("c2", "read", "b.rs"),
                call("c3", "write", "b.rs"),
                call("c4", "edit", "c.rs"),
                call("c5", "bash", "z.rs"),
            ]),
        },
    ];

    let (read, modified) = extract_file_ops(&entries);
    assert_eq!(read, vec!["a.rs".to_string(), "old.rs".to_string()]);
    assert_eq!(modified, vec!["b.rs".to_string(), "c.rs".to_string()]);
}

#[tokio::test]
async fn long_history_compacts_and_context_shrinks() {
    let dir = tmp();
    let mut session = Session::create(&dir, "/x").unwrap();
    long_history(&mut session, 10);

    let before = build_context(&session.entries, session.leaf_id.as_deref());
    assert!(before.len() > 5);

    let llm = mock(vec![summary_events("MOCK SUMMARY")]);
    let done = compact(&mut session, &llm, &settings(), None)
        .await
        .unwrap();
    assert!(done);

    match last_compaction(&session) {
        Some(EntryKind::Compaction {
            summary,
            tokens_before,
            first_kept_id,
            ..
        }) => {
            assert_eq!(summary, "MOCK SUMMARY");
            assert!(*tokens_before > 0);
            assert!(!first_kept_id.is_empty());
        }
        other => panic!("expected compaction entry, got {other:?}"),
    }

    let after = build_context(&session.entries, session.leaf_id.as_deref());
    assert!(after.len() < before.len());
    match &after[0] {
        Message::User { text } => assert!(text.contains("MOCK SUMMARY")),
        other => panic!("expected summary first, got {other:?}"),
    }
}

#[tokio::test]
async fn second_compaction_sends_previous_summary() {
    let dir = tmp();
    let mut session = Session::create(&dir, "/x").unwrap();
    long_history(&mut session, 10);

    let llm = mock(vec![summary_events("FIRST SUMMARY")]);
    assert!(compact(&mut session, &llm, &settings(), None)
        .await
        .unwrap());

    long_history(&mut session, 10);

    let (llm, seen) = mock_capture(vec![summary_events("SECOND SUMMARY")]);
    assert!(compact(&mut session, &llm, &settings(), None)
        .await
        .unwrap());

    let requests = seen.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(request.contains("<previous-summary>"));
    assert!(request.contains("FIRST SUMMARY"));
    assert!(request.contains("Update the existing summary"));

    match last_compaction(&session) {
        Some(EntryKind::Compaction { summary, .. }) => assert_eq!(summary, "SECOND SUMMARY"),
        other => panic!("expected compacted entry, got {other:?}"),
    }
}

#[tokio::test]
async fn custom_focus_reaches_the_summary_request() {
    let dir = tmp();
    let mut session = Session::create(&dir, "/x").unwrap();
    long_history(&mut session, 10);

    let (llm, seen) = mock_capture(vec![summary_events("S")]);
    assert!(
        compact(&mut session, &llm, &settings(), Some("keep the API names"))
            .await
            .unwrap()
    );

    let requests = seen.lock().unwrap();
    assert!(requests[0].contains("Additional focus: keep the API names"));
}

#[tokio::test]
async fn nothing_to_cut_returns_false() {
    let dir = tmp();
    let mut session = Session::create(&dir, "/x").unwrap();
    session.append(user("only one")).unwrap();

    let llm = mock(vec![]);
    assert!(!compact(&mut session, &llm, &settings(), None)
        .await
        .unwrap());
    assert!(session.entries.last().is_some());
}

#[tokio::test]
async fn agent_compacts_after_a_long_turn() {
    let dir = tmp();
    let mut session = Session::create(&dir, "/x").unwrap();
    long_history(&mut session, 10);

    let llm = mock(vec![
        vec![StreamEvent::Done(Message::Assistant {
            blocks: vec![Block::Text {
                text: "done".to_string(),
            }],
            provider: "mock".to_string(),
            model: "m".to_string(),
            usage: None,
            stop: StopReason::Stop,
        })],
        summary_events("AUTO SUMMARY"),
    ]);

    let cfg = AgentConfig {
        cwd: dir.clone(),
        system_prompt: "test".to_string(),
        context_window: 100,
        compaction: settings(),
        limits: crate::cost::Limits::default(),
        approver: crate::safety::Approver::Auto,
        allow_risky: false,
        approvals: crate::safety::Approvals::default(),
        guard: crate::safety::Guard {
            cwd: dir.clone(),
            allow_paths: Vec::new(),
            pirs_home: dir.join(".pirs-home"),
            user_home: None,
        },
    };
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    drop(rx);
    run_prompt(
        &mut session,
        &llm,
        &cfg,
        "go".to_string(),
        tx,
        tokio::sync::mpsc::unbounded_channel().1,
        tokio::sync::mpsc::channel(1).1,
    )
    .await
    .unwrap();

    match session.entries.last().map(|e| &e.kind) {
        Some(EntryKind::Compaction { summary, .. }) => assert_eq!(summary, "AUTO SUMMARY"),
        other => panic!("expected automatic compaction, got {other:?}"),
    }
}
