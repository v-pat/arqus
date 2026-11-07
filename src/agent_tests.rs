use super::*;
use crate::safety::{Approvals, Approver, Decision, Guard};
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
        limits: Limits::default(),
        approver: Approver::Auto,
        allow_risky: false,
        approvals: Approvals::default(),
        guard: Guard {
            cwd: cwd.to_path_buf(),
            allow_paths: Vec::new(),
            pirs_home: cwd.join(".pirs-home"),
            user_home: Some(cwd.join(".home")),
        },
    }
}

async fn run(script: Vec<Vec<StreamEvent>>, cwd: &std::path::Path) -> (Session, Llm) {
    let (session, llm, result) = run_allowing_error(script, cwd).await;
    result.unwrap();
    (session, llm)
}

async fn run_allowing_error(
    script: Vec<Vec<StreamEvent>>,
    cwd: &std::path::Path,
) -> (Session, Llm, anyhow::Result<()>) {
    let dir = temp_dir();
    let mut session = Session::create(&dir, "/x").unwrap();
    let llm = mock(script);
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    drop(rx); // nobody listens: sends fail fast instead of blocking
    let (_steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel();
    let result = run_prompt(
        &mut session,
        &llm,
        &config(cwd),
        "go".to_string(),
        tx,
        steer_rx,
        tokio::sync::mpsc::channel(1).1,
    )
    .await;
    (session, llm, result)
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
    let (session, _llm, result) = run_allowing_error(script, &cwd).await;
    assert!(result.is_err(), "a failed turn must return Err");

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

#[tokio::test]
async fn two_queued_messages_join_the_next_request_in_order() {
    let cwd = temp_dir();
    std::fs::create_dir_all(&cwd).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let script = vec![
        vec![done(
            vec![call(
                "call_1",
                "bash",
                json!({"command": "sleep 0.4; echo hi"}),
            )],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("done")], StopReason::Stop)],
    ];
    let llm = Llm::MockCapture(
        Arc::new(Mutex::new(std::collections::VecDeque::from(script))),
        seen.clone(),
    );
    let mut session = Session::create(&temp_dir(), "/x").unwrap();
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    drop(rx);
    let (steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel();
    let cfg = config(&cwd);

    let handle = tokio::spawn(async move {
        run_prompt(
            &mut session,
            &llm,
            &cfg,
            "go".to_string(),
            tx,
            steer_rx,
            tokio::sync::mpsc::channel(1).1,
        )
        .await
    });
    // The tool round is running: queue two messages.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    steer_tx.send("second".to_string()).unwrap();
    steer_tx.send("third".to_string()).unwrap();
    handle.await.unwrap().unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    let request: Value = serde_json::from_str(&seen[1]).unwrap();
    let users: Vec<String> = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| {
            if m["role"] == "user" {
                m["text"].as_str().map(str::to_string)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        users,
        vec!["go".to_string(), "second".to_string(), "third".to_string()],
        "queued messages must follow the tool result, in order"
    );
}

#[tokio::test]
async fn leftover_steering_after_the_final_answer_starts_a_new_turn() {
    let dir = temp_dir();
    let mut session = Session::create(&dir, "/x").unwrap();
    let _ = session
        .append(EntryKind::Message {
            message: Message::User {
                text: "go".to_string(),
            },
        })
        .unwrap();
    let final_id = session
        .append(EntryKind::Message {
            message: Message::Assistant {
                blocks: vec![text("first answer")],
                provider: "mock".to_string(),
                model: "m".to_string(),
                usage: None,
                stop: StopReason::Stop,
            },
        })
        .unwrap();

    let (steer_tx, mut steer_rx) = tokio::sync::mpsc::unbounded_channel();
    steer_tx.send("next question".to_string()).unwrap();
    let (events, rx) = tokio::sync::mpsc::channel(8);
    drop(rx);

    let added = drain_steering(&mut session, &mut steer_rx, &events).unwrap();
    assert_eq!(added, 1, "a leftover message must make the loop continue");

    match &session.entries.last().unwrap().kind {
        EntryKind::Message {
            message: Message::User { text },
        } => assert_eq!(text, "next question"),
        other => panic!("expected a new user entry, got {other:?}"),
    }
    assert_eq!(
        session.entries.last().unwrap().parent_id.as_deref(),
        Some(final_id.as_str()),
        "the queued message must follow the final answer"
    );
}

#[tokio::test]
async fn abort_mid_tool_still_builds_a_legal_history() {
    let cwd = temp_dir();
    std::fs::create_dir_all(&cwd).unwrap();
    let script = vec![vec![done(
        vec![call(
            "call_1",
            "bash",
            json!({"command": "sleep 2; echo late"}),
        )],
        StopReason::ToolUse,
    )]];
    let mut session = Session::create(&temp_dir(), "/x").unwrap();
    let path = session.path.clone();
    let llm = mock(script);
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let drainer = tokio::spawn(async move {
        let mut rx = rx;
        while rx.recv().await.is_some() {}
    });
    let (_steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel();
    let cfg = config(&cwd);

    let handle = tokio::spawn(async move {
        let _ = run_prompt(
            &mut session,
            &llm,
            &cfg,
            "go".to_string(),
            tx,
            steer_rx,
            tokio::sync::mpsc::channel(1).1,
        )
        .await;
    });

    // Wait until the tool call is on disk, then abort like the TUI does.
    let mut found = false;
    for _ in 0..200 {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.contains("\"tool_call\"") {
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(found, "the tool call must reach the session file");
    handle.abort();
    let _ = handle.await;
    drainer.abort();

    // Reopen the file, then build the context the next request would send.
    let reloaded = Session::open(&path).unwrap();
    let messages = build_context(&reloaded.entries, reloaded.leaf_id.as_deref());
    let calls: Vec<String> = messages
        .iter()
        .flat_map(|m| match m {
            Message::Assistant { blocks, .. } => blocks
                .iter()
                .filter_map(|b| match b {
                    Block::ToolCall { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect();
    let results: Vec<String> = messages
        .iter()
        .filter_map(|m| match m {
            Message::ToolResult { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();
    assert!(!calls.is_empty());
    assert_eq!(
        calls, results,
        "every aborted tool call needs a synthetic result"
    );
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

async fn run_interactive(
    script: Vec<Vec<StreamEvent>>,
    cwd: &std::path::Path,
    decisions: Vec<Decision>,
) -> (Session, Vec<AgentEvent>) {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let mut session = Session::create(&dir, "/x").unwrap();
    let llm = mock(script);
    let (events, mut event_rx) = tokio::sync::mpsc::channel(64);
    let collector = tokio::spawn(async move {
        let mut seen = Vec::new();
        while let Some(event) = event_rx.recv().await {
            seen.push(event);
        }
        seen
    });
    let (approval_tx, approval_rx) = tokio::sync::mpsc::channel(8);
    for decision in decisions {
        approval_tx.send(decision).await.unwrap();
    }
    drop(approval_tx);
    let (_steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut cfg = config(cwd);
    cfg.approver = Approver::Interactive;
    run_prompt(
        &mut session,
        &llm,
        &cfg,
        "go".to_string(),
        events,
        steer_rx,
        approval_rx,
    )
    .await
    .unwrap();
    let seen = collector.await.unwrap();
    (session, seen)
}

fn last_tool_result(session: &Session) -> (String, bool) {
    session
        .entries
        .iter()
        .rev()
        .find_map(|entry| match &entry.kind {
            EntryKind::Message {
                message: Message::ToolResult { text, is_error, .. },
            } => Some((text.clone(), *is_error)),
            _ => None,
        })
        .expect("a tool result")
}

fn approval_tools(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ApprovalRequest { tool, .. } => Some(tool.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn denied_write_is_not_written() {
    let cwd = temp_dir();
    std::fs::create_dir_all(&cwd).unwrap();
    let script = vec![
        vec![done(
            vec![call(
                "call_1",
                "write",
                json!({"path": "denied.txt", "content": "no"}),
            )],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("ok")], StopReason::Stop)],
    ];
    let (session, events) = run_interactive(script, &cwd, vec![Decision::Deny]).await;

    assert!(
        !cwd.join("denied.txt").exists(),
        "denied write must not run"
    );
    let (text, is_error) = last_tool_result(&session);
    assert!(is_error);
    assert_eq!(text, "The user denied this tool call.");
    assert_eq!(approval_tools(&events), vec!["write".to_string()]);
}

#[tokio::test]
async fn allowed_write_runs() {
    let cwd = temp_dir();
    std::fs::create_dir_all(&cwd).unwrap();
    let script = vec![
        vec![done(
            vec![call(
                "call_1",
                "write",
                json!({"path": "allowed.txt", "content": "yes"}),
            )],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("ok")], StopReason::Stop)],
    ];
    let (session, _events) = run_interactive(script, &cwd, vec![Decision::Allow]).await;

    assert_eq!(
        std::fs::read_to_string(cwd.join("allowed.txt")).unwrap(),
        "yes"
    );
    let (_, is_error) = last_tool_result(&session);
    assert!(!is_error);
}

#[tokio::test]
async fn allow_always_is_remembered_for_one_tool_only() {
    let cwd = temp_dir();
    std::fs::create_dir_all(&cwd).unwrap();
    let script = vec![
        vec![done(
            vec![
                call(
                    "call_1",
                    "write",
                    json!({"path": "one.txt", "content": "1"}),
                ),
                call(
                    "call_2",
                    "write",
                    json!({"path": "two.txt", "content": "2"}),
                ),
                call("call_3", "bash", json!({"command": "echo hi"})),
            ],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("ok")], StopReason::Stop)],
    ];
    // Only two answers: write is allowed for the session, bash still asks.
    let (_session, events) =
        run_interactive(script, &cwd, vec![Decision::AllowAlways, Decision::Allow]).await;

    assert!(cwd.join("one.txt").exists());
    assert!(cwd.join("two.txt").exists(), "write must not ask twice");
    assert_eq!(
        approval_tools(&events),
        vec!["write".to_string(), "bash".to_string()]
    );
}
