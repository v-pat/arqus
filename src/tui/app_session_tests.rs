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
