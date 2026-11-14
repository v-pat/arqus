use super::*;
use serde_json::json;

#[test]
fn read_summary_shows_the_line_range() {
    assert_eq!(
        summary_for("read", &json!({"path": "src/a.rs", "limit": 200})),
        "read src/a.rs (lines 1-200)"
    );
    assert_eq!(
        summary_for("read", &json!({"path": "a", "offset": 5, "limit": 10})),
        "read a (lines 5-14)"
    );
    assert_eq!(
        summary_for("read", &json!({"path": "a"})),
        "read a (lines 1-2000)"
    );
}

#[test]
fn write_summary_counts_lines() {
    assert_eq!(
        summary_for("write", &json!({"path": "a.rs", "content": "one\ntwo\n"})),
        "write a.rs (2 lines)"
    );
    assert_eq!(
        summary_for("write", &json!({"path": "a.rs", "content": ""})),
        "write a.rs (0 lines)"
    );
}

#[test]
fn edit_summary_counts_removed_and_added_lines() {
    assert_eq!(
        summary_for(
            "edit",
            &json!({"path": "util.py", "old_text": "a\nb\nc", "new_text": "x"})
        ),
        "edit util.py (-3 +1)"
    );
}

#[test]
fn bash_summary_keeps_the_first_line_and_truncates() {
    assert_eq!(
        summary_for("bash", &json!({"command": "python3 -m unittest\nsecond"})),
        "bash python3 -m unittest"
    );
    let long = "x".repeat(80);
    let summary = summary_for("bash", &json!({"command": long}));
    assert!(summary.ends_with('…'));
    assert!(summary.chars().count() <= 56);
}

#[test]
fn mcp_summary_shows_server_tool_and_args() {
    assert_eq!(
        summary_for("mcp__serena__get_symbols", &json!({"path": "src/main.rs"})),
        "mcp:serena/get_symbols src/main.rs"
    );
    assert_eq!(
        summary_for(
            "mcp__serena__find_references",
            &json!({"symbol_name": "AgentConfig"})
        ),
        "mcp:serena/find_references \"AgentConfig\""
    );
}

#[test]
fn truncate_width_is_display_width_aware() {
    assert_eq!(truncate_width("abc", 10), "abc");
    assert_eq!(truncate_width("abcdef", 4), "abc…");
    assert_eq!(truncate_width("你好世界", 4), "你…");
}

#[test]
fn a_live_tool_block_starts_running_and_finishes() {
    let mut tool = ToolBlock::new(1, "bash", &json!({"command": "ls"}));
    assert_eq!(tool.state, ToolState::Running);
    assert_eq!(tool.summary, "bash ls");
    tool.finish("out".to_string(), false);
    assert_eq!(tool.state, ToolState::Ok);
    assert_eq!(tool.output, "out");
    assert!(tool.elapsed.is_some());
}

#[test]
fn session_entries_build_user_assistant_tool_and_diff_blocks() {
    use crate::types::{Block as Part, EntryKind, Message, StopReason};

    let entries = vec![
        Entry {
            id: "1".to_string(),
            parent_id: None,
            ts: 0,
            kind: EntryKind::Message {
                message: Message::User {
                    text: "fix it".to_string(),
                },
            },
        },
        Entry {
            id: "2".to_string(),
            parent_id: Some("1".to_string()),
            ts: 1,
            kind: EntryKind::Message {
                message: Message::Assistant {
                    blocks: vec![
                        Part::Text {
                            text: "on it".to_string(),
                        },
                        Part::ToolCall {
                            id: "call_1".to_string(),
                            name: "read".to_string(),
                            args: json!({"path": "a.rs"}),
                        },
                    ],
                    provider: "test".to_string(),
                    model: "m".to_string(),
                    usage: None,
                    stop: StopReason::ToolUse,
                },
            },
        },
        Entry {
            id: "3".to_string(),
            parent_id: Some("2".to_string()),
            ts: 2,
            kind: EntryKind::Message {
                message: Message::ToolResult {
                    call_id: "call_1".to_string(),
                    tool_name: "read".to_string(),
                    text: "file body".to_string(),
                    is_error: false,
                },
            },
        },
        Entry {
            id: "4".to_string(),
            parent_id: Some("3".to_string()),
            ts: 3,
            kind: EntryKind::Compaction {
                summary: "sum".to_string(),
                first_kept_id: "3".to_string(),
                tokens_before: 900,
                read_files: Vec::new(),
                modified_files: Vec::new(),
                summary_usage: None,
            },
        },
    ];
    let blocks = blocks_from_entries(&entries);
    assert_eq!(blocks.len(), 4);
    assert!(matches!(&blocks[0], Block::User(text) if text == "fix it"));
    assert!(matches!(&blocks[1], Block::Assistant { text, .. } if text == "on it"));
    match &blocks[2] {
        Block::Tool(tool) => {
            assert_eq!(tool.summary, "read a.rs (lines 1-2000)");
            assert_eq!(tool.output, "file body");
            assert_eq!(tool.state, ToolState::Ok);
        }
        other => panic!("expected a tool block, got {other:?}"),
    }
    assert!(matches!(
        &blocks[3],
        Block::Compaction { tokens_before: 900 }
    ));
}

#[test]
fn a_very_long_line_is_cut_before_it_reaches_the_screen() {
    let long = "x".repeat(100_000);
    let cut = cap_lines(&long);
    assert_eq!(cut.chars().count(), MAX_LINE_CHARS + 1);
    assert!(cut.ends_with('…'));

    let mut tool = ToolBlock::new(0, "bash", &json!({ "command": "cat big" }));
    tool.finish(format!("short\n{long}\nshort"), false);
    assert!(tool
        .output
        .lines()
        .all(|line| line.chars().count() <= MAX_LINE_CHARS + 1));
    assert_eq!(tool.output.lines().count(), 3);
}

#[test]
fn blocks_from_entries_cut_long_tool_output() {
    let long = "y".repeat(100_000);
    let entries = vec![Entry {
        id: "e1".to_string(),
        parent_id: None,
        ts: 0,
        kind: EntryKind::Message {
            message: Message::ToolResult {
                call_id: "call_1".to_string(),
                tool_name: "bash".to_string(),
                text: long,
                is_error: false,
            },
        },
    }];
    let blocks = blocks_from_entries(&entries);
    match &blocks[0] {
        Block::Tool(tool) => {
            assert_eq!(tool.output.chars().count(), MAX_LINE_CHARS + 1);
            assert!(tool.output.ends_with('…'));
        }
        other => panic!("expected a tool block, got {other:?}"),
    }
}
