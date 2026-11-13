use std::time::Duration;

use serde_json::json;

use crate::tui::app::App;
use crate::tui::block::{Block, ToolBlock, ToolState};
use crate::tui::footer::ProviderState;
use crate::tui::theme::{ColorDepth, Theme};
use crate::tui::widgets::transcript::render_block;

fn demo_app(blocks: Vec<Block>) -> App {
    let mut app = App::new(
        std::path::PathBuf::from("/tmp"),
        "/tmp".to_string(),
        "test-model".to_string(),
        128_000,
    );
    app.home = "/Users/me".to_string();
    app.provider_state = ProviderState::Connected {
        provider: "openrouter".to_string(),
        model: "test-model".to_string(),
    };
    app.blocks = blocks;
    app.cache.clear();
    app.set_size(80, 24);
    app.refresh_render();
    app
}

fn tool(name: &str, args: serde_json::Value, output: &str, state: ToolState) -> Block {
    let mut tool = ToolBlock::new(0, name, &args);
    tool.output = output.to_string();
    tool.state = state;
    tool.elapsed = if state == ToolState::Running {
        None
    } else {
        Some(Duration::from_millis(1200))
    };
    Block::Tool(tool)
}

fn block_texts(block: &Block, width: usize, expanded: bool, show_thinking: bool) -> Vec<String> {
    let theme = Theme::dark(ColorDepth::True);
    render_block(block, width, &theme, expanded, show_thinking)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

fn bash_output() -> String {
    (1..=12)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn tool_output_is_cut_to_five_lines_when_collapsed() {
    let block = tool(
        "bash",
        json!({"command": "python3 -m unittest"}),
        &bash_output(),
        ToolState::Ok,
    );
    let texts = block_texts(&block, 80, false, false);
    assert!(texts[0].starts_with("● bash  python3 -m unittest"));
    assert_eq!(texts[1], "│ line 1");
    assert_eq!(texts[5], "│ line 5");
    assert_eq!(texts[6], "│ … +7 lines (ctrl+o to expand)");
    assert_eq!(texts.len(), 7);
}

#[test]
fn expanded_tool_output_shows_until_two_hundred_lines() {
    let output: String = (1..=250)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let block = tool("bash", json!({"command": "x"}), &output, ToolState::Ok);
    let texts = block_texts(&block, 80, true, false);
    assert_eq!(texts[1], "│ line 1");
    assert_eq!(texts[200], "│ line 200");
    assert_eq!(texts[201], "│ … +50 more lines");
    assert_eq!(texts.len(), 202);
}

#[test]
fn an_edit_card_draws_the_diff() {
    let block = tool(
        "edit",
        json!({
            "path": "util.py",
            "old_text": "return text.lower().replace(\" \", \"_\")",
            "new_text": "return text.lower().replace(\" \", \"-\")"
        }),
        "Edited util.py",
        ToolState::Ok,
    );
    let texts = block_texts(&block, 80, false, false);
    assert!(texts[0].contains("util.py (-1 +1)"));
    assert_eq!(texts[1], "│ - return text.lower().replace(\" \", \"_\")");
    assert_eq!(texts[2], "│ + return text.lower().replace(\" \", \"-\")");
}

#[test]
fn a_long_diff_stops_at_twelve_lines_each() {
    let old = (1..=20)
        .map(|i| format!("old {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let new = (1..=15)
        .map(|i| format!("new {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let block = tool(
        "edit",
        json!({"path": "a", "old_text": old, "new_text": new}),
        "",
        ToolState::Ok,
    );
    let texts = block_texts(&block, 80, false, false);
    assert_eq!(texts[1], "│ - old 1");
    assert_eq!(texts[12], "│ - old 12");
    assert_eq!(texts[13], "│ … +8 more");
    assert!(texts.iter().any(|line| line == "│ + new 12"));
    assert!(texts.iter().any(|line| line == "│ … +3 more"));
}

#[test]
fn a_write_card_shows_eight_dim_lines() {
    let content = (1..=10)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let block = tool(
        "write",
        json!({"path": "a.rs", "content": content}),
        "Wrote",
        ToolState::Ok,
    );
    let texts = block_texts(&block, 80, false, false);
    assert!(texts[0].contains("a.rs (10 lines)"));
    assert_eq!(texts[8], "│ line 8");
    assert_eq!(texts[9], "│ … +2 more lines");
}

#[test]
fn the_state_icon_sits_at_the_right_edge() {
    let running = block_texts(
        &tool("bash", json!({"command": "ls"}), "", ToolState::Running),
        40,
        false,
        false,
    );
    assert_eq!(running[0].chars().count(), 40);
    assert!(running[0].ends_with('⠋'));

    let done = block_texts(
        &tool("bash", json!({"command": "ls"}), "", ToolState::Ok),
        40,
        false,
        false,
    );
    assert!(done[0].ends_with("✓ 1.2s"));
    assert_eq!(done[0].chars().count(), 40);
}

#[test]
fn running_tool_snapshot() {
    let mut app = demo_app(vec![
        Block::User("list the files".to_string()),
        Block::Assistant {
            text: "Checking.".to_string(),
            streaming: false,
        },
        tool("bash", json!({"command": "ls -la"}), "", ToolState::Running),
    ]);
    app.running = true;
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("tool-running-80x24", &actual);
}

#[test]
fn bash_tool_collapsed_snapshot() {
    let mut app = demo_app(vec![
        Block::User("run the tests".to_string()),
        tool(
            "bash",
            json!({"command": "python3 -m unittest"}),
            &bash_output(),
            ToolState::Ok,
        ),
    ]);
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("tool-bash-collapsed-80x24", &actual);
}

#[test]
fn bash_tool_expanded_snapshot() {
    let mut app = demo_app(vec![
        Block::User("run the tests".to_string()),
        tool(
            "bash",
            json!({"command": "python3 -m unittest"}),
            &bash_output(),
            ToolState::Ok,
        ),
    ]);
    app.toggle_expanded();
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("tool-bash-expanded-80x24", &actual);
}

#[test]
fn edit_diff_snapshot() {
    let mut app = demo_app(vec![
        Block::User("fix the slug".to_string()),
        tool(
            "edit",
            json!({
                "path": "util.py",
                "old_text": "return text.lower().replace(\" \", \"_\")",
                "new_text": "return text.lower().replace(\" \", \"-\")"
            }),
            "Edited util.py",
            ToolState::Ok,
        ),
        Block::Assistant {
            text: "Fixed: slugify now uses `-`. The tests pass.".to_string(),
            streaming: false,
        },
    ]);
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("tool-edit-diff-80x24", &actual);
}

#[test]
fn error_tool_snapshot() {
    let mut app = demo_app(vec![
        Block::User("run it".to_string()),
        tool(
            "bash",
            json!({"command": "python3 missing.py"}),
            "python3: can't open file 'missing.py': [Errno 2] No such file or directory",
            ToolState::Err,
        ),
    ]);
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("tool-error-80x24", &actual);
}
