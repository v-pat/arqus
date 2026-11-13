//! `/export`: write the visible conversation as Markdown.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::block::Block;

/// Default file name: `arqus-session-<date>.md` in the project folder.
pub fn default_path(cwd: &str) -> PathBuf {
    Path::new(cwd).join(format!(
        "arqus-session-{}.md",
        crate::system_prompt::today_utc()
    ))
}

/// Render the blocks the user can see. Later turns stay below earlier ones.
pub fn render(blocks: &[Block]) -> String {
    let mut out = String::from("# arqus session\n");
    for block in blocks {
        match block {
            Block::User(text) => out.push_str(&format!("\n## You\n\n{text}\n")),
            Block::Assistant { text, .. } => out.push_str(&format!("\n## Assistant\n\n{text}\n")),
            Block::Thinking { text } => out.push_str(&format!("\n### Thinking\n\n{text}\n")),
            Block::Tool(tool) => {
                out.push_str(&format!("\n### Tool: {}\n\n{}\n", tool.name, tool.summary));
                if !tool.output.trim().is_empty() {
                    out.push_str(&format!("\n```\n{}\n```\n", tool.output.trim_end()));
                }
            }
            Block::Notice(text, _) => out.push_str(&format!("\n> {text}\n")),
            Block::Compaction { tokens_before } => out.push_str(&format!(
                "\n> context compacted ({tokens_before} tokens before)\n"
            )),
        }
    }
    out
}

/// Write the export. A relative `name` is resolved against `cwd`.
pub fn write(blocks: &[Block], cwd: &str, name: Option<&str>) -> Result<PathBuf> {
    let path = match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            let candidate = Path::new(name);
            if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                Path::new(cwd).join(candidate)
            }
        }
        None => default_path(cwd),
    };
    std::fs::write(&path, render(blocks))
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
