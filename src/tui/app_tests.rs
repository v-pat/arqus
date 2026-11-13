use super::*;
use crate::agent::AgentEvent;
use crate::tui::block::{Block, ToolState};
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

fn line_text(line: &ratatui::text::Line) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn text_deltas_merge_into_one_block() {
    let mut app = app();
    app.running = true;
    app.apply_event(AgentEvent::TextDelta("he".to_string()));
    app.apply_event(AgentEvent::TextDelta("llo".to_string()));

    assert_eq!(app.blocks.len(), 1);
    match &app.blocks[0] {
        Block::Assistant { text, streaming } => {
            assert_eq!(text, "hello");
            assert!(streaming);
        }
        other => panic!("expected an assistant block, got {other:?}"),
    }
}

#[test]
fn tool_start_and_end_make_one_block() {
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

    assert_eq!(app.blocks.len(), 1);
    match &app.blocks[0] {
        Block::Tool(tool) => {
            assert_eq!(tool.summary, "read a.rs (lines 1-2000)");
            assert_eq!(tool.output, "file body");
            assert_eq!(tool.state, ToolState::Ok);
            assert!(tool.elapsed.is_some());
        }
        other => panic!("expected a tool block, got {other:?}"),
    }
}

#[test]
fn a_tool_end_without_a_start_still_makes_a_block() {
    let mut app = app();
    app.apply_event(AgentEvent::ToolEnd {
        name: "bash".to_string(),
        text: "out".to_string(),
        is_error: false,
    });
    assert_eq!(app.blocks.len(), 1);
    assert!(matches!(&app.blocks[0], Block::Tool(tool) if tool.state == ToolState::Ok));
}

#[test]
fn turn_done_stops_running_and_closes_the_stream() {
    let mut app = app();
    app.running = true;
    app.apply_event(AgentEvent::TextDelta("hello".to_string()));
    app.apply_event(AgentEvent::TurnDone);

    assert!(!app.running);
    assert_eq!(app.status, "ready");
    assert!(matches!(
        &app.blocks[0],
        Block::Assistant {
            streaming: false,
            ..
        }
    ));
}

#[test]
fn text_after_a_tool_starts_a_new_block() {
    let mut app = app();
    app.apply_event(AgentEvent::TextDelta("first".to_string()));
    app.apply_event(AgentEvent::ToolStart {
        name: "bash".to_string(),
        args: json!({}),
    });
    app.apply_event(AgentEvent::TextDelta("second".to_string()));

    assert_eq!(app.blocks.len(), 3);
    assert!(matches!(&app.blocks[0], Block::Assistant { text, .. } if text == "first"));
    assert!(matches!(&app.blocks[2], Block::Assistant { text, .. } if text == "second"));
}

#[test]
fn an_error_event_becomes_a_red_notice() {
    let mut app = app();
    app.apply_event(AgentEvent::Error("boom\nsecond".to_string()));

    assert_eq!(app.blocks.len(), 1);
    assert!(matches!(&app.blocks[0], Block::Notice(text, Level::Error) if text.contains("boom")));
}

#[test]
fn thinking_deltas_merge_and_survive_a_text_block() {
    let mut app = app();
    app.apply_event(AgentEvent::ThinkingDelta("one ".to_string()));
    app.apply_event(AgentEvent::ThinkingDelta("two".to_string()));
    app.apply_event(AgentEvent::TextDelta("done".to_string()));

    assert_eq!(app.blocks.len(), 2);
    assert!(matches!(&app.blocks[0], Block::Thinking { text } if text == "one two"));
}

#[test]
fn scrolling_follows_until_the_user_scrolls_up() {
    let mut app = app();
    app.set_page(2);
    for i in 0..10 {
        app.info(format!("line {i}"));
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
        app.info(format!("line {i}"));
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
        app.info(format!("line {i}"));
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
fn resize_rerenders_long_lines() {
    let mut app = app();
    app.set_page(5);
    app.set_width(80);
    app.push_block(Block::Assistant {
        text: "x".repeat(100),
        streaming: false,
    });
    assert_eq!(app.wrapped.len(), 2);

    app.set_width(20);
    assert_eq!(app.wrapped.len(), 5);
}

#[test]
fn spinner_and_elapsed_time_follow_the_ticks() {
    let mut app = app();
    app.running = true;
    app.on_tick();
    assert_eq!(app.spinner(), "⠙");
    for _ in 0..9 {
        app.on_tick();
    }
    assert_eq!(app.elapsed_secs(), 1);
    assert_eq!(app.spinner(), "⠋");
}

#[test]
fn user_blocks_keep_their_bar_after_wrapping() {
    let mut app = app();
    app.set_width(10);
    app.push_user("alpha beta gamma");

    let texts: Vec<String> = app.wrapped.iter().map(line_text).collect();
    assert_eq!(texts[0], "┃ alpha");
    assert!(texts.len() > 1);
    assert!(texts[1].starts_with("┃ "));
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
fn info_blocks_keep_the_welcome_screen() {
    let mut app = app();
    app.info("note");
    assert!(!app.has_conversation());
    app.push_user("hi");
    assert!(app.has_conversation());
}

#[test]
fn compaction_alone_keeps_the_welcome_screen() {
    let mut app = app();
    app.apply_event(AgentEvent::Compacted { tokens_before: 900 });
    assert!(!app.has_conversation());
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
    assert!(matches!(app.blocks.last(), Some(Block::User(text)) if text == "hello"));
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
