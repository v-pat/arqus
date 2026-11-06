//! The `write` tool.

use std::path::Path;

use serde_json::Value;

use super::{arg_str, resolve_path, ToolOutput};

pub fn run(args: &Value, cwd: &Path) -> ToolOutput {
    let path = match arg_str(args, "path") {
        Ok(path) => path,
        Err(err) => return ToolOutput::error(err),
    };
    let content = match arg_str(args, "content") {
        Ok(content) => content,
        Err(err) => return ToolOutput::error(err),
    };

    let full = resolve_path(cwd, &path);
    if let Some(parent) = full.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            return ToolOutput::error(format!(
                "could not create folder {}: {err}",
                parent.display()
            ));
        }
    }
    if let Err(err) = std::fs::write(&full, &content) {
        return ToolOutput::error(format!("could not write {}: {err}", full.display()));
    }

    ToolOutput::ok(format!("Wrote {} bytes to {path}", content.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_byte_count() {
        let dir = std::env::temp_dir().join(format!("pirs_write_{}", std::process::id()));
        let out = run(&json!({"path": "a/b.txt", "content": "hi"}), &dir);
        assert!(!out.is_error, "{}", out.text);
        assert_eq!(out.text, "Wrote 2 bytes to a/b.txt");
        assert_eq!(std::fs::read_to_string(dir.join("a/b.txt")).unwrap(), "hi");
        std::fs::remove_dir_all(dir).ok();
    }
}
