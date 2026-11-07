use super::*;
use crate::agent::AgentEvent;
use serde_json::json;
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
fn text_deltas_merge_into_one_line() {
    let mut app = app();
    app.running = true;
    app.apply_event(AgentEvent::TextDelta("he".to_string()));
    app.apply_event(AgentEvent::TextDelta("llo".to_string()));

    assert_eq!(app.lines.len(), 1);
    assert_eq!(app.lines[0].kind, LineKind::Assistant);
    assert_eq!(app.lines[0].text, "hello");
}

#[test]
fn tool_start_and_end_make_two_lines() {
    let mut app = app();
    app.apply_event(AgentEvent::ToolStart {
        name: "read".to_string(),
        args: json!({"path": "a.rs"}),
    });
    app.apply_event(AgentEvent::ToolEnd {
        name: "read".to_string(),
        text: "file body".to_string(),
        is_error: false,
    });

    assert_eq!(app.lines.len(), 2);
    assert_eq!(app.lines[0].kind, LineKind::ToolCall);
    assert!(app.lines[0].text.contains("read"));
    assert_eq!(app.lines[1].kind, LineKind::ToolResult);
    assert!(app.lines[1].text.contains("file body"));
}

#[test]
fn tool_result_is_cut_to_ten_lines() {
    let mut app = app();
    let text: String = (0..25).map(|i| format!("line {i}\n")).collect();
    app.apply_event(AgentEvent::ToolEnd {
        name: "bash".to_string(),
        text,
        is_error: false,
    });

    assert_eq!(app.lines.len(), 11);
    assert!(app.lines[10].text.contains("15 more lines"));
}

#[test]
fn turn_done_stops_running() {
    let mut app = app();
    app.running = true;
    app.apply_event(AgentEvent::TurnDone);

    assert!(!app.running);
    assert_eq!(app.status, "ready");
}

#[test]
fn text_after_a_tool_starts_a_new_line() {
    let mut app = app();
    app.apply_event(AgentEvent::TextDelta("first".to_string()));
    app.apply_event(AgentEvent::ToolStart {
        name: "bash".to_string(),
        args: json!({}),
    });
    app.apply_event(AgentEvent::TextDelta("second".to_string()));

    assert_eq!(app.lines.len(), 3);
    assert_eq!(app.lines[0].text, "first");
    assert_eq!(app.lines[2].text, "second");
}

#[test]
fn error_is_red_material_and_can_be_multiline() {
    let mut app = app();
    app.apply_event(AgentEvent::ToolEnd {
        name: "bash".to_string(),
        text: "boom\nsecond".to_string(),
        is_error: true,
    });

    assert_eq!(app.lines.len(), 2);
    assert_eq!(app.lines[0].kind, LineKind::Error);
    assert!(app.lines[0].text.contains("boom"));
}

#[test]
fn scrolling_follows_until_the_user_scrolls_up() {
    let mut app = app();
    app.set_page(2);
    for i in 0..10 {
        app.push(LineKind::Info, format!("line {i}"));
    }
    assert_eq!(app.scroll, 8);

    app.scroll_up(2);
    assert_eq!(app.scroll, 6);
    assert!(!app.follow);

    app.scroll_down(2);
    assert_eq!(app.scroll, 8);
    assert!(app.follow);
}

#[test]
fn home_and_end_move_the_view() {
    let mut app = app();
    app.set_page(2);
    for i in 0..10 {
        app.push(LineKind::Info, format!("line {i}"));
    }

    app.scroll_top();
    assert_eq!(app.scroll, 0);
    assert!(!app.follow);

    app.end_follow();
    assert_eq!(app.scroll, 8);
    assert!(app.follow);
}

#[test]
fn streaming_stays_put_while_the_user_reads_history() {
    let mut app = app();
    app.set_page(2);
    for i in 0..6 {
        app.push(LineKind::Info, format!("line {i}"));
    }
    app.scroll_up(2);
    let frozen = app.scroll;

    app.running = true;
    app.apply_event(AgentEvent::TextDelta("more".to_string()));
    assert_eq!(app.scroll, frozen);
    assert!(!app.follow);

    app.end_follow();
    assert_eq!(app.scroll, app.wrapped.len() - 2);
}

#[test]
fn resize_rewraps_long_lines() {
    let mut app = app();
    app.set_page(5);
    app.set_width(80);
    app.push(LineKind::Assistant, "x".repeat(100));
    assert_eq!(app.wrapped.len(), 2);

    app.set_width(20);
    assert_eq!(app.wrapped.len(), 5);
}

#[test]
fn spinner_and_elapsed_time_follow_the_ticks() {
    let mut app = app();
    app.running = true;
    app.on_tick();
    assert_eq!(app.spinner(), "/");
    for _ in 0..9 {
        app.on_tick();
    }
    assert_eq!(app.elapsed_secs(), 1);
    assert_eq!(app.spinner(), "-");
}

#[test]
fn code_fence_content_is_dim() {
    let mut app = app();
    app.push(LineKind::Assistant, "before".to_string());
    app.push(LineKind::Assistant, "```".to_string());
    app.push(LineKind::Assistant, "let x = 1;".to_string());
    app.push(LineKind::Assistant, "```".to_string());
    app.push(LineKind::Assistant, "after".to_string());

    let dims: Vec<bool> = app.wrapped.iter().map(|l| l.dim).collect();
    assert_eq!(dims, vec![false, false, true, false, false]);
}

#[test]
fn user_lines_keep_their_prefix_after_wrapping() {
    let mut app = app();
    app.set_width(10);
    app.push_user("alpha beta gamma");

    let texts: Vec<&str> = app.wrapped.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts[0], "> alpha");
    assert!(texts.len() > 1);
}

#[test]
fn ctrl_c_idle_needs_a_second_press_inside_the_window() {
    let mut first = app();
    assert!(!first.ctrl_c_idle());
    assert_eq!(first.status, "press Ctrl+C again to quit");
    first.on_tick();
    assert!(first.ctrl_c_idle());

    let mut second = app();
    assert!(!second.ctrl_c_idle());
    for _ in 0..22 {
        second.on_tick();
    }
    assert_eq!(second.status, "ready");
    assert!(!second.ctrl_c_idle());
}

#[test]
fn queued_prompts_wait_until_the_agent_accepts_them() {
    let mut app = app();
    app.running = true;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.steer_tx = Some(tx);

    assert!(app.queue_prompt("hello"));
    assert_eq!(app.queue, vec!["hello".to_string()]);
    assert_eq!(rx.try_recv().unwrap(), "hello");

    app.apply_event(AgentEvent::UserMessage("hello".to_string()));
    assert!(app.queue.is_empty());
    assert_eq!(app.lines.last().unwrap().kind, LineKind::User);
    assert_eq!(app.lines.last().unwrap().text, "hello");
}

#[test]
fn queue_prompt_without_a_run_is_rejected() {
    let mut app = app();
    assert!(!app.queue_prompt("hello"));
    assert!(app.queue.is_empty());
}

#[test]
fn clear_queue_drops_every_pending_prompt() {
    let mut app = app();
    app.queue.push("a".to_string());
    app.queue.push("b".to_string());
    assert_eq!(app.clear_queue(), 2);
    assert!(app.queue.is_empty());
}
