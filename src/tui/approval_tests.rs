use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio::sync::mpsc;

use super::app::App;
use super::block::{Block, ToolState};
use super::footer::{Mode, ProviderState};
use super::keys::{Key, KeyCodeX};
use super::msg::{Effect, Msg};
use super::running;
use super::testing::{assert_snapshot, render_to_string};
use super::update::update;
use crate::agent::{self, run_prompt, AgentConfig, AgentEvent};
use crate::ai::{Llm, StreamEvent};
use crate::compaction::CompactionSettings;
use crate::cost::Limits;
use crate::safety::{Approvals, Approver, Decision, Guard};
use crate::session::{new_id, Session};
use crate::types::Block as MessageBlock;
use crate::types::{Message, StopReason};

fn key(ch: char) -> Key {
    Key::new(KeyCodeX::Char(ch))
}

fn app() -> App {
    let mut app = App::new(
        PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    );
    app.provider_state = ProviderState::Connected {
        provider: "openrouter".to_string(),
        model: "test-model".to_string(),
    };
    app
}

fn approval_app(tool: &str, preview: String) -> App {
    let mut app = app();
    app.running = true;
    app.push_user("change the build");
    let _ = app.approval_request(tool.to_string(), preview);
    app
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("arqus_approval_{tag}_{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn approval_bash_snapshot() {
    let preview = agent::approval_preview(
        "bash",
        &json!({"command": "cargo build --release && ./target/release/arqus --help"}),
        None,
    );
    let mut app = approval_app("bash", preview);
    assert_snapshot("approval-bash-80x24", &render_to_string(&mut app, 80, 24));
}

#[test]
fn approval_write_snapshot() {
    let lines: Vec<String> = (1..=25).map(|n| format!("line {n}")).collect();
    let preview = agent::approval_preview(
        "write",
        &json!({"path": "src/new.rs", "content": lines.join("\n")}),
        None,
    );
    let mut app = approval_app("write", preview);
    assert_snapshot("approval-write-80x24", &render_to_string(&mut app, 80, 24));
}

#[test]
fn approval_edit_snapshot() {
    let preview = agent::approval_preview(
        "edit",
        &json!({
            "path": "src/main.rs",
            "old_text": "let answer = 41;",
            "new_text": "let answer = 42;"
        }),
        None,
    );
    let mut app = approval_app("edit", preview);
    assert_snapshot("approval-edit-80x24", &render_to_string(&mut app, 80, 24));
}

#[test]
fn approval_risky_snapshot_and_marker() {
    let preview = agent::approval_preview(
        "bash",
        &json!({"command": "rm -rf build && cargo build"}),
        Some("bulk delete"),
    );
    assert!(preview.starts_with("RISKY\n"));
    let mut app = approval_app("bash", preview);
    let screen = render_to_string(&mut app, 80, 24);
    assert!(screen.contains("RISKY"));
    assert_snapshot("approval-risky-80x24", &screen);
}

#[test]
fn the_modal_ignores_every_key_that_is_not_a_decision() {
    let mut app = approval_app("bash", "ls".to_string());
    let effects = update(&mut app, Msg::Key(key('?')));
    assert!(effects.is_empty());
    assert!(app.overlays.is_empty(), "? must not open help");
    assert!(app.approval.is_some());
    update(&mut app, Msg::Key(key('x')));
    assert!(app.approval.is_some(), "typing must not close it");
}

#[test]
fn decision_keys_send_the_expected_answer() {
    let mut allow = app();
    let (tx, mut rx) = mpsc::channel(4);
    allow.approval_tx = Some(tx);
    allow.approval_request("bash".to_string(), "ls".to_string());
    update(&mut allow, Msg::Key(key('y')));
    assert_eq!(rx.try_recv().unwrap(), Decision::Allow);
    assert!(allow.approval.is_none());

    let mut deny = app();
    let (tx, mut rx) = mpsc::channel(4);
    deny.approval_tx = Some(tx);
    deny.approval_request("bash".to_string(), "ls".to_string());
    assert!(update(&mut deny, Msg::Key(key('n'))).is_empty());
    assert_eq!(rx.try_recv().unwrap(), Decision::Deny);

    let mut always = app();
    let (tx, mut rx) = mpsc::channel(4);
    always.approval_tx = Some(tx);
    always.approval_request("write".to_string(), "path: x".to_string());
    update(&mut always, Msg::Key(key('a')));
    assert_eq!(rx.try_recv().unwrap(), Decision::AllowAlways);

    let mut stop = app();
    let (tx, mut rx) = mpsc::channel(4);
    stop.approval_tx = Some(tx);
    stop.approval_request("bash".to_string(), "ls".to_string());
    assert_eq!(
        update(&mut stop, Msg::Key(Key::new(KeyCodeX::Esc))),
        vec![Effect::Abort, Effect::RefreshTokens]
    );
    assert_eq!(rx.try_recv().unwrap(), Decision::Deny);
    assert!(stop.approval.is_none());
}

#[test]
fn the_bell_rings_only_after_ten_idle_seconds() {
    let mut idle = app();
    idle.running = true;
    idle.ticks = 150;
    idle.last_input_tick = 0;
    idle.approval_tx = Some(mpsc::channel(4).0);
    let effects = update(
        &mut idle,
        Msg::Agent(AgentEvent::ApprovalRequest {
            tool: "bash".to_string(),
            preview: "ls".to_string(),
        }),
    );
    assert_eq!(effects, vec![Effect::Bell]);

    let mut recent = app();
    recent.running = true;
    recent.ticks = 150;
    recent.last_input_tick = 150;
    recent.approval_tx = Some(mpsc::channel(4).0);
    let effects = update(
        &mut recent,
        Msg::Agent(AgentEvent::ApprovalRequest {
            tool: "bash".to_string(),
            preview: "ls".to_string(),
        }),
    );
    assert!(effects.is_empty());
}

#[test]
fn the_running_line_waits_for_the_answer() {
    let mut app = approval_app("bash", "ls".to_string());
    assert_eq!(
        running::line(&app),
        Some("waiting for your approval".to_string())
    );
    app.running = false;
    assert_eq!(running::line(&app), None);
}

fn done(blocks: Vec<MessageBlock>, stop: StopReason) -> StreamEvent {
    StreamEvent::Done(Message::Assistant {
        blocks,
        provider: "mock".to_string(),
        model: "m".to_string(),
        usage: None,
        stop,
    })
}

fn call(id: &str, name: &str, args: serde_json::Value) -> MessageBlock {
    MessageBlock::ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        args,
    }
}

fn text(value: &str) -> MessageBlock {
    MessageBlock::Text {
        text: value.to_string(),
    }
}

fn config(cwd: &Path) -> AgentConfig {
    AgentConfig {
        cwd: cwd.to_path_buf(),
        system_prompt: "test".to_string(),
        context_window: 128_000,
        compaction: CompactionSettings {
            enabled: false,
            ..CompactionSettings::default()
        },
        limits: Limits::default(),
        approver: Approver::Interactive,
        allow_risky: false,
        plan: false,
        approvals: Approvals::default(),
        guard: Guard {
            cwd: cwd.to_path_buf(),
            allow_paths: Vec::new(),
            arqus_home: cwd.join(".arqus-home"),
            user_home: Some(cwd.join(".home")),
        },
        mcp: None,
    }
}

/// Run the real agent loop with the mock provider, and feed every event
/// through `update` exactly like the terminal event loop does.
async fn drive(
    cwd: PathBuf,
    script: Vec<Vec<StreamEvent>>,
    answers: &[char],
) -> (App, Session, Vec<AgentEvent>) {
    let mut session = Session::create(&temp_dir("session"), "/x").unwrap();
    let llm = Llm::Mock(Arc::new(Mutex::new(VecDeque::from(script))));
    let (event_tx, mut event_rx) = mpsc::channel(64);
    let (approval_tx, approval_rx) = mpsc::channel(8);
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<String>();

    let mut app = app();
    app.running = true;
    app.approval_tx = Some(approval_tx);
    let cfg = config(&cwd);
    let mut answers: VecDeque<char> = answers.iter().copied().collect();
    let handle = tokio::spawn(async move {
        run_prompt(
            &mut session,
            &llm,
            &cfg,
            "go".to_string(),
            event_tx,
            steer_rx,
            approval_rx,
        )
        .await
        .unwrap();
        session
    });

    let mut seen = Vec::new();
    while let Some(event) = event_rx.recv().await {
        let approval = matches!(event, AgentEvent::ApprovalRequest { .. });
        let finished = matches!(event, AgentEvent::TurnDone);
        seen.push(event.clone());
        update(&mut app, Msg::Agent(event));
        if approval {
            let answer = answers.pop_front().expect("a scripted answer");
            update(&mut app, Msg::Key(key(answer)));
        }
        if finished {
            break;
        }
    }
    let session = handle.await.unwrap();
    (app, session, seen)
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

fn last_tool_result(session: &Session) -> (String, bool) {
    session
        .entries
        .iter()
        .rev()
        .find_map(|entry| match &entry.kind {
            crate::types::EntryKind::Message {
                message: Message::ToolResult { text, is_error, .. },
            } => Some((text.clone(), *is_error)),
            _ => None,
        })
        .expect("a tool result")
}

#[tokio::test]
async fn allow_once_runs_the_tool_through_the_update_path() {
    let cwd = temp_dir("allow");
    let script = vec![
        vec![done(
            vec![call("c1", "bash", json!({"command": "echo hi > made.txt"}))],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("done")], StopReason::Stop)],
    ];
    let (app, session, events) = drive(cwd.clone(), script, &['y']).await;

    assert!(cwd.join("made.txt").exists());
    assert_eq!(approval_tools(&events), vec!["bash".to_string()]);
    let (_, is_error) = last_tool_result(&session);
    assert!(!is_error);
    assert!(app
        .blocks
        .iter()
        .any(|block| matches!(block, Block::Tool(tool) if tool.state == ToolState::Ok)));
}

#[tokio::test]
async fn deny_returns_the_denied_text_and_does_not_run() {
    let cwd = temp_dir("deny");
    let script = vec![
        vec![done(
            vec![call(
                "c1",
                "write",
                json!({"path": "denied.txt", "content": "no"}),
            )],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("ok")], StopReason::Stop)],
    ];
    let (app, session, events) = drive(cwd.clone(), script, &['n']).await;

    assert!(!cwd.join("denied.txt").exists());
    assert_eq!(approval_tools(&events), vec!["write".to_string()]);
    assert_eq!(
        last_tool_result(&session),
        ("The user denied this tool call.".to_string(), true)
    );
    assert!(app.blocks.iter().any(|block| matches!(
        block,
        Block::Tool(tool)
            if tool.state == ToolState::Err
                && tool.output == "The user denied this tool call."
    )));
}

#[tokio::test]
async fn allow_always_is_remembered_for_the_same_tool() {
    let cwd = temp_dir("always");
    let script = vec![
        vec![done(
            vec![
                call("c1", "write", json!({"path": "one.txt", "content": "1"})),
                call("c2", "write", json!({"path": "two.txt", "content": "2"})),
                call("c3", "bash", json!({"command": "echo hi > three.txt"})),
            ],
            StopReason::ToolUse,
        )],
        vec![done(vec![text("done")], StopReason::Stop)],
    ];
    let (_, _, events) = drive(cwd.clone(), script, &['a', 'y']).await;

    assert!(cwd.join("one.txt").exists());
    assert!(cwd.join("two.txt").exists(), "write must not ask twice");
    assert!(cwd.join("three.txt").exists());
    assert_eq!(
        approval_tools(&events),
        vec!["write".to_string(), "bash".to_string()]
    );
}

#[test]
fn plan_mode_blocks_changes_and_reads_still_work() {
    // The engine half of plan mode is covered in `agent_tests`; this checks
    // that the TUI sends the mode to the engine as one effect.
    let mut app = app();
    assert_eq!(
        super::commands::set_mode(&mut app, Mode::Plan),
        vec![Effect::SetMode(Mode::Plan)]
    );
    assert_eq!(app.mode, Mode::Plan);
}
