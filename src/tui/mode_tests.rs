use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio::sync::mpsc;

use super::app::App;
use super::block::{Block, ToolState};
use super::footer::Mode;
use super::keys::{Key, KeyCodeX};
use super::msg::Msg;
use super::theme::Theme;
use super::update::update;
use super::view::border_color;
use crate::agent::{run_prompt, AgentConfig, AgentEvent, PLAN_REFUSAL};
use crate::ai::{Llm, StreamEvent};
use crate::safety::{Approver, Guard};
use crate::session::Session;
use crate::types::Block as MessageBlock;
use crate::types::{Message, StopReason};

fn key(ch: char) -> Key {
    Key::new(KeyCodeX::Char(ch))
}

fn app() -> App {
    App::new(PathBuf::from("/tmp"), "/tmp".into(), "m".into(), 128_000)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("arqus_m_{tag}_{}", crate::session::new_id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn call(id: &str, name: &str, args: serde_json::Value) -> MessageBlock {
    MessageBlock::ToolCall {
        id: id.into(),
        name: name.into(),
        args,
    }
}

fn txt(t: &str) -> Vec<MessageBlock> {
    vec![MessageBlock::Text {
        text: t.to_string(),
    }]
}

fn done(blocks: Vec<MessageBlock>, stop: StopReason) -> StreamEvent {
    StreamEvent::Done(Message::Assistant {
        blocks,
        provider: "t".to_string(),
        model: "m".to_string(),
        usage: None,
        stop,
    })
}

fn config(cwd: &Path, mode: Mode) -> AgentConfig {
    let approver = match mode {
        Mode::Ask | Mode::Plan => Approver::Interactive,
        Mode::Auto => Approver::Auto,
    };
    AgentConfig {
        cwd: cwd.to_path_buf(),
        system_prompt: "test".to_string(),
        context_window: 128_000,
        compaction: Default::default(),
        limits: Default::default(),
        approver,
        allow_risky: false,
        plan: mode == Mode::Plan,
        approvals: Default::default(),
        guard: Guard {
            cwd: cwd.to_path_buf(),
            allow_paths: Vec::new(),
            arqus_home: cwd.join(".arqus"),
            user_home: None,
        },
        mcp: None,
    }
}

async fn drive_mode(
    cwd: PathBuf,
    script: Vec<Vec<StreamEvent>>,
    answers: &[char],
    mode: Mode,
) -> (App, Session, Vec<AgentEvent>) {
    let mut session = Session::create(&temp_dir("sess"), "/x").unwrap();
    let llm = Llm::Mock(Arc::new(Mutex::new(VecDeque::from(script))));
    let (event_tx, mut event_rx) = mpsc::channel(64);
    let (approval_tx, approval_rx) = mpsc::channel(8);
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel::<String>();

    let mut app = app();
    app.mode = mode;
    app.running = true;
    app.approval_tx = Some(approval_tx);
    let cfg = config(&cwd, mode);
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
            update(
                &mut app,
                Msg::Key(key(answers.pop_front().expect("answer"))),
            );
        }
        if finished {
            break;
        }
    }
    (app, handle.await.unwrap(), seen)
}

fn tool_states(app: &App) -> Vec<ToolState> {
    app.blocks
        .iter()
        .filter_map(|b| match b {
            Block::Tool(t) => Some(t.state),
            _ => None,
        })
        .collect()
}

fn approval_count(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ApprovalRequest { .. }))
        .count()
}

#[tokio::test]
async fn plan_mode_blocks_changes_and_allows_reads_without_asking() {
    let cwd = temp_dir("plan");
    std::fs::write(cwd.join("readable.txt"), "hello plan").unwrap();
    let script = vec![
        vec![done(
            vec![
                call("c1", "read", json!({"path": "readable.txt"})),
                call("c2", "write", json!({"path": "new.txt", "content": "1"})),
                call(
                    "c3",
                    "edit",
                    json!({"path": "readable.txt", "old": "hello", "new": "bye"}),
                ),
                call("c4", "bash", json!({"command": "echo hi > shell.txt"})),
            ],
            StopReason::ToolUse,
        )],
        vec![done(txt("plan ready"), StopReason::Stop)],
    ];

    let (app, _s, events) = drive_mode(cwd.clone(), script, &[], Mode::Plan).await;
    assert_eq!(approval_count(&events), 0, "plan mode must never prompt");
    assert!(!cwd.join("new.txt").exists());
    assert!(!cwd.join("shell.txt").exists());
    assert_eq!(
        std::fs::read_to_string(cwd.join("readable.txt")).unwrap(),
        "hello plan"
    );

    let tools: Vec<(&str, ToolState, &str)> = app
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Tool(t) => Some((t.name.as_str(), t.state, t.output.as_str())),
            _ => None,
        })
        .collect();

    assert_eq!(tools.len(), 4);
    assert_eq!(tools[0].0, "read");
    assert_eq!(tools[0].1, ToolState::Ok);
    assert!(tools[0].2.contains("hello plan"));
    for (name, state, output) in &tools[1..] {
        assert_ne!(*name, "read");
        assert_eq!(*state, ToolState::Err);
        assert_eq!(*output, PLAN_REFUSAL);
    }
}

#[tokio::test]
async fn auto_mode_executes_tools_without_approval_popups() {
    let cwd = temp_dir("auto");
    let script = vec![
        vec![done(
            vec![
                call(
                    "c1",
                    "write",
                    json!({"path": "auto_file.txt", "content": "automated"}),
                ),
                call("c2", "bash", json!({"command": "echo ran > auto_bash.txt"})),
            ],
            StopReason::ToolUse,
        )],
        vec![done(txt("all done"), StopReason::Stop)],
    ];

    let (app, _s, events) = drive_mode(cwd.clone(), script, &[], Mode::Auto).await;
    assert_eq!(approval_count(&events), 0);
    assert!(cwd.join("auto_file.txt").exists());
    assert!(cwd.join("auto_bash.txt").exists());
    assert_eq!(tool_states(&app), vec![ToolState::Ok, ToolState::Ok]);
}

#[tokio::test]
async fn auto_mode_blocks_risky_commands_without_allow_risky() {
    let cwd = temp_dir("auto_risky");
    let script = vec![
        vec![done(
            vec![call("c1", "bash", json!({"command": "sudo rm -rf /"}))],
            StopReason::ToolUse,
        )],
        vec![done(txt("stopped"), StopReason::Stop)],
    ];

    let (app, _s, events) = drive_mode(cwd.clone(), script, &[], Mode::Auto).await;
    assert_eq!(approval_count(&events), 0);
    let tool = app
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Tool(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(tool.state, ToolState::Err);
    assert!(tool.output.contains("refused risky command"));
}

#[tokio::test]
async fn ask_mode_prompts_for_changes_and_allows_reads() {
    let cwd = temp_dir("ask");
    std::fs::write(cwd.join("read.txt"), "read content").unwrap();
    let script = vec![
        vec![done(
            vec![
                call("c1", "read", json!({"path": "read.txt"})),
                call(
                    "c2",
                    "write",
                    json!({"path": "ask_out.txt", "content": "ok"}),
                ),
            ],
            StopReason::ToolUse,
        )],
        vec![done(txt("all done"), StopReason::Stop)],
    ];

    let (app, _s, events) = drive_mode(cwd.clone(), script, &['y'], Mode::Ask).await;
    assert_eq!(approval_count(&events), 1);
    assert!(cwd.join("ask_out.txt").exists());
    assert_eq!(tool_states(&app), vec![ToolState::Ok, ToolState::Ok]);
}

#[test]
fn mode_presentation_and_border_colors() {
    let theme = Theme::dark(super::theme::ColorDepth::True);
    assert_eq!(
        (Mode::Ask.chip(), border_color(Mode::Ask, &theme)),
        ("ask", theme.border)
    );
    assert_eq!(
        (Mode::Auto.chip(), border_color(Mode::Auto, &theme)),
        ("auto", theme.warning)
    );
    assert_eq!(
        (Mode::Plan.chip(), border_color(Mode::Plan, &theme)),
        ("plan", theme.accent)
    );
}
