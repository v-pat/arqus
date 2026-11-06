use super::*;
use crate::session::new_id;
use crate::types::Entry;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("pirs_agent_{}", new_id()))
}

fn done(blocks: Vec<Block>, stop: StopReason) -> StreamEvent {
    StreamEvent::Done(Message::Assistant {
        blocks,
        provider: "mock".to_string(),
        model: "m".to_string(),
        usage: None,
        stop,
    })
}

fn call(id: &str, name: &str, args: Value) -> Block {
    Block::ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args,
    }
}

fn text(t: &str) -> Block {
    Block::Text {
        text: t.to_string(),
    }
}

fn mock(script: Vec<Vec<StreamEvent>>) -> Llm {
    Llm::Mock(Arc::new(Mutex::new(std::collections::VecDeque::from(
        script,
    ))))
}

fn config(cwd: &std::path::Path) -> AgentConfig {
    AgentConfig {
        cwd: cwd.to_path_buf(),
        system_prompt: "test".to_string(),
        context_window: 128_000,
        compaction: CompactionSettings {
            enabled: false,
            ..CompactionSettings::default()
        },
    }
}

async fn run(script: Vec<Vec<StreamEvent>>, cwd: &std::path::Path) -> (Session, Llm) {
    let dir = temp_dir();
    let mut session = Session::create(&dir, "/x").unwrap();
    let llm = mock(script);
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    drop(rx); // nobody listens: sends fail fast instead of blocking
    run_prompt(&mut session, &llm, &config(cwd), "go".to_string(), tx)
        .await
        .unwrap();
    (session, llm)
}

fn entry(session: &Session, index: usize) -> &Entry {
    &session.entries[index]
}

#[tokio::test]
async fn tool_call_then_final_text() {
    let cwd = temp_dir();
    std::fs::create_dir_all(&cwd).unwrap();
    let script = vec![
        vec![done(
            vec![call("call_1", "bash", json!({"command": "echo hi"}))],
            StopReason::ToolUse,
        )],
        vec![
            StreamEvent::TextDelta("done".to_string()),
            done(vec![text("done")], StopReason::Stop),
        ],
    ];
    let (session, _llm) = run(script, &cwd).await;

    assert_eq!(session.entries.len(), 4);
    assert!(matches!(
        &entry(&session, 0).kind,
        EntryKind::Message {
            message: Message::User { .. }
        }
    ));
    match &entry(&session, 1).kind {
        EntryKind::Message {
            message: Message::Assistant { blocks, stop, .. },
        } => {
            assert_eq!(*stop, StopReason::ToolUse);
            assert!(matches!(blocks[0], Block::ToolCall { .. }));
        }
        other => panic!("expected assistant tool call, got {other:?}"),
    }
    match &entry(&session, 2).kind {
        EntryKind::Message {
            message: Message::ToolResult { text, is_error, .. },
        } => {
            assert!(text.contains("hi"), "tool result was {text:?}");
            assert!(!is_error);
        }
        other => panic!("expected tool result, got {other:?}"),
    }
    match &entry(&session, 3).kind {
        EntryKind::Message {
            message: Message::Assistant { blocks, .. },
        } => assert!(matches!(blocks[0], Block::Text { .. })),
        other => panic!("expected final assistant text, got {other:?}"),
    }
}

#[tokio::test]
async fn unknown_tool_is_an_error_and_the_loop_continues() {
    let cwd = temp_dir();
    let script = vec![
        vec![done(
            vec![call("call_1", "nope", json!({}))],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("recovered")], StopReason::Stop)],
    ];
    let (session, _llm) = run(script, &cwd).await;

    assert_eq!(session.entries.len(), 4);
    match &entry(&session, 2).kind {
        EntryKind::Message {
            message: Message::ToolResult { is_error, text, .. },
        } => {
            assert!(is_error);
            assert!(text.contains("unknown tool"));
        }
        other => panic!("expected error tool result, got {other:?}"),
    }
    assert!(matches!(
        &entry(&session, 3).kind,
        EntryKind::Message {
            message: Message::Assistant { .. }
        }
    ));
}

#[tokio::test]
async fn provider_error_ends_the_turn() {
    let cwd = temp_dir();
    let script = vec![vec![StreamEvent::Error("boom".to_string())]];
    let (session, _llm) = run(script, &cwd).await;

    assert_eq!(session.entries.len(), 2);
    match &entry(&session, 1).kind {
        EntryKind::Message {
            message: Message::Assistant { stop, blocks, .. },
        } => {
            assert_eq!(*stop, StopReason::Error);
            assert!(blocks.is_empty());
        }
        other => panic!("expected failed assistant entry, got {other:?}"),
    }
}

#[test]
fn switch_model_appends_a_model_change_entry() {
    let dir = temp_dir();
    let mut session = Session::create(&dir, "/x").unwrap();
    let mut runtime = Runtime {
        llm: mock(vec![]),
        cfg: config(&dir),
        model_name: "old-model".to_string(),
    };

    runtime
        .switch_model(&mut session, "anthropic:claude-test")
        .unwrap();

    assert_eq!(runtime.model_name, "claude-test");
    assert!(matches!(runtime.llm, Llm::Anthropic(_)));
    assert!(runtime.cfg.context_window > 0);
    match &session.entries.last().unwrap().kind {
        EntryKind::ModelChange { provider, model } => {
            assert_eq!(provider, "anthropic");
            assert_eq!(model, "claude-test");
        }
        other => panic!("expected model_change entry, got {other:?}"),
    }
}
