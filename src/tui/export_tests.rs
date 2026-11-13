use super::*;
use crate::tui::block::ToolBlock;

#[test]
fn export_writes_a_readable_markdown_document() {
    let blocks = vec![
        Block::User("list the files".to_string()),
        Block::Assistant {
            text: "Checking.".to_string(),
            streaming: false,
        },
        Block::Tool(ToolBlock {
            id: 0,
            call_id: "c1".to_string(),
            name: "bash".to_string(),
            summary: "bash ls".to_string(),
            state: crate::tui::block::ToolState::Ok,
            output: "Cargo.toml\nsrc\n".to_string(),
            started: std::time::Instant::now(),
            elapsed: None,
            args: serde_json::Value::Null,
        }),
        Block::Notice("note".to_string(), crate::tui::msg::Level::Info),
    ];
    let text = render(&blocks);
    assert_eq!(
        text,
        "# arqus session\n\n## You\n\nlist the files\n\n## Assistant\n\nChecking.\n\n### Tool: bash\n\nbash ls\n\n```\nCargo.toml\nsrc\n```\n\n> note\n"
    );
}

#[test]
fn write_uses_the_given_path_and_the_default_name() {
    let dir = std::env::temp_dir().join(format!("arqus_export_{}", crate::session::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cwd = dir.to_string_lossy().to_string();
    let blocks = vec![Block::User("hi".to_string())];

    let named = write(&blocks, &cwd, Some("out.md")).unwrap();
    assert_eq!(named, dir.join("out.md"));
    assert!(std::fs::read_to_string(&named).unwrap().contains("hi"));

    let default = write(&blocks, &cwd, None).unwrap();
    let name = default.file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with("arqus-session-"));
    assert!(name.ends_with(".md"));
    let _ = std::fs::remove_dir_all(dir);
}
