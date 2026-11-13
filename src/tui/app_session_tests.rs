use super::*;
use crate::agent::AgentEvent;
use crate::safety::Decision;
use std::path::Path;

fn app() -> App {
    App::new(
        Path::new("/tmp").to_path_buf(),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    )
}

#[test]
fn resume_lists_sessions_newest_first_and_picks() {
    let dir = std::env::temp_dir().join(format!("arqus_tui_{}", crate::session::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("100_a.jsonl"), "").unwrap();
    std::fs::write(dir.join("300_c.jsonl"), "").unwrap();
    std::fs::write(dir.join("200_b.jsonl"), "").unwrap();

    let mut app = App::new(
        dir.clone(),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    );
    assert!(app.start_resume());
    assert_eq!(app.resume.as_ref().unwrap().picker.match_count(), 3);

    app.move_resume(1);
    let picked = app.pick_resume().unwrap();
    assert_eq!(picked.file_name().unwrap().to_string_lossy(), "200_b.jsonl");
    assert!(app.resume.is_none());
}

#[test]
fn tree_picker_moves_the_leaf_and_shows_the_path() {
    use crate::session::Session;
    use crate::types::{EntryKind, Message};

    let dir = std::env::temp_dir().join(format!("arqus_tui_{}", crate::session::new_id()));
    let mut session = Session::create(&dir, "/tmp").unwrap();
    session
        .append(EntryKind::Message {
            message: Message::User {
                text: "root".to_string(),
            },
        })
        .unwrap();
    let second = session
        .append(EntryKind::Message {
            message: Message::User {
                text: "second".to_string(),
            },
        })
        .unwrap();

    let mut app = app();
    app.set_session(session);
    assert!(app.start_tree());
    assert_eq!(app.tree.as_ref().unwrap().picker.match_count(), 2);

    app.move_tree(1);
    let path = app.select_tree().unwrap();
    assert_eq!(path.split(" > ").count(), 2);
    assert_eq!(
        app.session.as_ref().unwrap().leaf_id.as_deref(),
        Some(second.as_str())
    );
    assert!(app.tree.is_none());
}

#[test]
fn approval_request_opens_and_answers_the_modal() {
    let mut app = app();
    app.running = true;
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    app.approval_tx = Some(tx);

    app.apply_event(AgentEvent::ApprovalRequest {
        tool: "bash".to_string(),
        preview: "RISKY\n\nrm -rf /".to_string(),
    });
    assert!(app.approval.is_some());
    assert_eq!(app.status, "approval needed: bash");

    assert!(app.answer_approval(Decision::Allow));
    assert_eq!(rx.try_recv().unwrap(), Decision::Allow);
    assert!(app.approval.is_none());
    assert!(!app.answer_approval(Decision::Deny), "no request pending");
}

#[test]
fn turn_done_clears_a_pending_approval() {
    let mut app = app();
    app.apply_event(AgentEvent::ApprovalRequest {
        tool: "write".to_string(),
        preview: "path: x".to_string(),
    });
    assert!(app.approval.is_some());
    app.apply_event(AgentEvent::TurnDone);
    assert!(app.approval.is_none());
}

#[test]
fn replay_session_builds_the_same_blocks_from_entries() {
    use crate::session::Session;
    use crate::types::{Message, StopReason};

    let dir = std::env::temp_dir().join(format!("arqus_tui_{}", crate::session::new_id()));
    let mut session = Session::create(&dir, "/tmp").unwrap();
    session
        .append(crate::types::EntryKind::Message {
            message: Message::User {
                text: "hello".to_string(),
            },
        })
        .unwrap();
    session
        .append(crate::types::EntryKind::Message {
            message: Message::Assistant {
                blocks: vec![crate::types::Block::Text {
                    text: "hi".to_string(),
                }],
                provider: "test".to_string(),
                model: "m".to_string(),
                usage: None,
                stop: StopReason::Stop,
            },
        })
        .unwrap();

    let mut app = app();
    app.set_session(session);
    app.replay_session();

    assert_eq!(app.blocks.len(), 2);
    assert!(matches!(&app.blocks[0], Block::User(text) if text == "hello"));
    assert!(matches!(&app.blocks[1], Block::Assistant { text, .. } if text == "hi"));
    assert!(app.has_conversation());
}

fn branch_session() -> crate::session::Session {
    use crate::session::Session;
    use crate::types::{EntryKind, Message};

    let dir = std::env::temp_dir().join(format!("arqus_tui_{}", crate::session::new_id()));
    let mut session = Session::create(&dir, "/tmp").unwrap();
    let user = |text: &str| EntryKind::Message {
        message: Message::User {
            text: text.to_string(),
        },
    };
    let root = session.append(user("root")).unwrap();
    session.append(user("main branch")).unwrap();
    session.set_leaf(&root).unwrap();
    session.append(user("side branch")).unwrap();
    session
}

#[test]
fn moving_the_leaf_rebuilds_only_the_new_path() {
    let mut app = app();
    app.set_session(branch_session());
    app.replay_session();
    assert_eq!(app.blocks.len(), 2);

    assert!(app.start_tree());
    app.move_tree(1);
    assert!(app.select_tree().is_some());
    let texts: Vec<String> = app
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::User(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["root", "main branch"]);
}

#[test]
fn rename_appends_a_custom_entry_and_updates_the_title() {
    let mut app = app();
    app.set_session(branch_session());
    crate::tui::commands::dispatch(&mut app, "/rename release prep");
    assert_eq!(app.session_title, "release prep");
    let last = app.session.as_ref().unwrap().entries.last().unwrap();
    assert!(matches!(
        &last.kind,
        crate::types::EntryKind::Custom { custom_type, data }
            if custom_type == "session_name" && data["name"] == "release prep"
    ));
    assert!(app
        .toasts
        .iter()
        .any(|toast| toast.text == "Session renamed to release prep"));
}

#[test]
fn delete_asks_then_returns_the_path() {
    let dir = std::env::temp_dir().join(format!("arqus_tui_{}", crate::session::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("100_a.jsonl"), "").unwrap();
    std::fs::write(dir.join("200_b.jsonl"), "").unwrap();
    let mut app = App::new(dir, "/tmp".to_string(), "test-model".to_string(), 128_000);
    assert!(app.start_resume());
    assert!(app.ask_delete_resume());
    assert!(app.resume_confirming());
    let path = app.answer_delete_resume(true).unwrap();
    assert!(path.ends_with("200_b.jsonl"));
    assert!(!app.resume_confirming());
}

fn write_session(path: &std::path::Path, text: &str) {
    std::fs::write(
        path,
        format!(
            "{{\"type\":\"session\",\"version\":1,\"id\":\"t\",\"cwd\":\"/tmp\",\"ts\":0}}\n{{\"id\":\"a\",\"parent_id\":null,\"ts\":0,\"type\":\"message\",\"message\":{{\"role\":\"user\",\"text\":\"{text}\"}}}}\n"
        ),
    )
    .unwrap();
}

#[test]
fn sessions_picker_snapshot() {
    let dir = std::env::temp_dir().join(format!("arqus_tui_{}", crate::session::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_session(&dir.join("100_a.jsonl"), "fix the failing test");
    write_session(&dir.join("200_b.jsonl"), "release prep");
    let mut app = App::new(
        dir.clone(),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    );
    app.set_session(crate::session::Session::open(&dir.join("100_a.jsonl")).unwrap());
    assert!(app.start_resume());
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("sessions-picker-80x24", &actual);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tree_picker_snapshot_with_a_branch() {
    let mut app = app();
    app.set_session(branch_session());
    assert!(app.start_tree());
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("tree-picker-branch-80x24", &actual);
}

#[test]
fn rename_toast_snapshot() {
    let mut app = app();
    app.set_session(branch_session());
    crate::tui::commands::dispatch(&mut app, "/rename release prep");
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("rename-toast-80x24", &actual);
}

#[test]
fn double_escape_on_an_empty_box_opens_the_tree() {
    let mut app = app();
    app.set_session(branch_session());
    let keys = |key| crate::tui::msg::Msg::Key(crate::tui::keys::Key::new(key));
    let mut effects = crate::tui::update::update(&mut app, keys(crate::tui::keys::KeyCodeX::Esc));
    assert!(effects.is_empty());
    effects = crate::tui::update::update(&mut app, keys(crate::tui::keys::KeyCodeX::Esc));
    assert!(effects.is_empty());
    assert!(app.tree.is_some());
}
