use std::time::Duration;

use ratatui::text::Line;
use serde_json::json;

use super::*;
use crate::tui::app::App;
use crate::tui::block::{Block, ToolBlock, ToolState};
use crate::tui::footer::ProviderState;
use crate::tui::msg::Effect;

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

fn line_text(line: &Line) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn block_texts(block: &Block, width: usize, expanded: bool, show_thinking: bool) -> Vec<String> {
    let theme = crate::tui::theme::Theme::dark(crate::tui::theme::ColorDepth::True);
    render_block(block, width, &theme, expanded, show_thinking)
        .iter()
        .map(line_text)
        .collect()
}

fn bash_output() -> String {
    (1..=12)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn markdown_headings_bullets_code_and_quotes() {
    let text = "# Title\n- one\n1. first\n> quote\nrun `ls`\n```\nlet x = 1;\n```";
    let texts = block_texts(
        &Block::Assistant {
            text: text.to_string(),
            streaming: false,
        },
        80,
        false,
        false,
    );
    assert_eq!(texts[0], "Title");
    assert_eq!(texts[1], "• one");
    assert_eq!(texts[2], "1. first");
    assert_eq!(texts[3], "> quote");
    assert_eq!(texts[4], "run ls");
    assert_eq!(texts[5], "│ let x = 1;");
}

#[test]
fn an_unclosed_fence_still_gets_a_gutter() {
    let texts = block_texts(
        &Block::Assistant {
            text: "before\n```\nlet x".to_string(),
            streaming: true,
        },
        80,
        false,
        false,
    );
    assert_eq!(texts[1], "│ let x");
}

#[test]
fn thinking_is_one_line_until_shown() {
    let block = Block::Thinking {
        text: "check the tests first".to_string(),
    };
    let hidden = block_texts(&block, 80, false, false);
    assert_eq!(hidden, vec!["▸ Thinking... (ctrl+t)"]);
    let shown = block_texts(&block, 80, false, true);
    assert_eq!(shown, vec!["▸ check the tests first"]);
}

#[test]
fn the_cache_is_keyed_by_width_and_content() {
    let mut app = demo_app(vec![Block::Assistant {
        text: "x".repeat(100),
        streaming: false,
    }]);
    assert_eq!(app.cache.len(), 1);
    assert_eq!(app.cache[0].key.unwrap().width, 80);
    assert_eq!(app.cache[0].lines.len(), 2);

    app.set_width(40);
    assert_eq!(app.cache[0].key.unwrap().width, 40);
    assert_eq!(app.cache[0].lines.len(), 3);

    let before = app.cache[0].key;
    app.blocks[0] = Block::Assistant {
        text: "short".to_string(),
        streaming: false,
    };
    app.refresh_render();
    assert_ne!(app.cache[0].key, before);
    assert_eq!(app.cache[0].lines.len(), 1);
}

#[test]
fn toggles_change_the_cache_key_and_the_output() {
    let output = bash_output();
    let mut app = demo_app(vec![tool(
        "bash",
        json!({"command": "ls"}),
        &output,
        ToolState::Ok,
    )]);
    assert_eq!(app.cache[0].lines.len(), 7);
    app.toggle_expanded();
    assert!(app.cache[0].key.unwrap().expanded);
    assert_eq!(app.cache[0].lines.len(), 13);
    app.toggle_expanded();
    assert_eq!(app.cache[0].lines.len(), 7);
}

#[test]
fn ctrl_t_and_ctrl_o_toggle_and_save_the_state() {
    let output = bash_output();
    let mut app = demo_app(vec![
        Block::Thinking {
            text: "why".to_string(),
        },
        tool("bash", json!({"command": "ls"}), &output, ToolState::Ok),
    ]);

    let effects = crate::tui::testing::replay(&mut app, "ctrl+t");
    assert!(app.show_thinking);
    assert_eq!(effects, vec![Effect::SaveState]);

    let effects = crate::tui::testing::replay(&mut app, "ctrl+o");
    assert!(app.expanded);
    assert!(effects.is_empty());

    crate::tui::testing::replay(&mut app, "ctrl+t ctrl+o");
    assert!(!app.show_thinking);
    assert!(!app.expanded);
}

#[test]
fn chat_with_markdown_snapshot() {
    let mut app = demo_app(vec![
        Block::User("fix the failing test in util.py".to_string()),
        Block::Assistant {
            text: "I'll run the tests first.\n\n## Plan\n\n- run `cargo test`\n- edit **util.py**\n\n```python\nreturn text.lower().replace(\" \", \"-\")\n```\n\nDone.".to_string(),
            streaming: false,
        },
    ]);
    app.context_tokens = 15_360;
    app.session_cost = Some(0.004);
    let actual = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("chat-markdown-80x24", &actual);
}

#[test]
fn thinking_block_snapshots() {
    let mut app = demo_app(vec![
        Block::User("why does it fail?".to_string()),
        Block::Thinking {
            text: "The test reads the old fixture. I should check the file first.".to_string(),
        },
        Block::Assistant {
            text: "Let me check the fixture.".to_string(),
            streaming: false,
        },
    ]);
    let hidden = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("thinking-hidden-80x24", &hidden);

    app.toggle_thinking();
    let shown = crate::tui::testing::render_to_string(&mut app, 80, 24);
    crate::tui::testing::assert_snapshot("thinking-shown-80x24", &shown);
}
