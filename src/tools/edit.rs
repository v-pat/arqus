//! The `edit` tool: one unique exact replacement.

use std::path::Path;

use serde_json::Value;

use super::{arg_str, resolve_path, ToolOutput};

pub fn run(args: &Value, cwd: &Path) -> ToolOutput {
    let path = match arg_str(args, "path") {
        Ok(path) => path,
        Err(err) => return ToolOutput::error(err),
    };
    let old_text = match arg_str(args, "old_text") {
        Ok(text) => text,
        Err(err) => return ToolOutput::error(err),
    };
    let new_text = match arg_str(args, "new_text") {
        Ok(text) => text,
        Err(err) => return ToolOutput::error(err),
    };

    let full = resolve_path(cwd, &path);
    let content = match std::fs::read_to_string(&full) {
        Ok(content) => content,
        Err(err) => return ToolOutput::error(format!("could not read {}: {err}", full.display())),
    };

    let count = content.matches(&old_text).count();
    if count == 0 {
        return ToolOutput::error(format!(
            "old_text was not found in {path}. Read the file and copy the exact lines."
        ));
    }
    if count > 1 {
        return ToolOutput::error(format!(
            "old_text was found {count} times in {path}; add more surrounding lines to make it unique"
        ));
    }

    let updated = content.replacen(&old_text, &new_text, 1);
    if let Err(err) = std::fs::write(&full, updated) {
        return ToolOutput::error(format!("could not write {}: {err}", full.display()));
    }

    ToolOutput::ok(format!("Edited {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_file(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pirs_edit_{}_{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), content).unwrap();
        dir
    }

    #[test]
    fn zero_matches_is_error() {
        let dir = temp_file("zero.txt", "hello\n");
        let out = run(
            &json!({"path": "zero.txt", "old_text": "nope", "new_text": "x"}),
            &dir,
        );
        assert!(out.is_error);
        assert!(out.text.contains("not found"), "{}", out.text);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn two_matches_is_error() {
        let dir = temp_file("two.txt", "a\na\n");
        let out = run(
            &json!({"path": "two.txt", "old_text": "a", "new_text": "b"}),
            &dir,
        );
        assert!(out.is_error);
        assert!(out.text.contains("2 times"), "{}", out.text);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn one_match_edits_file() {
        let dir = temp_file("one.txt", "a\nb\nc\n");
        let out = run(
            &json!({"path": "one.txt", "old_text": "b", "new_text": "B"}),
            &dir,
        );
        assert!(!out.is_error, "{}", out.text);
        assert_eq!(out.text, "Edited one.txt");
        assert_eq!(
            std::fs::read_to_string(dir.join("one.txt")).unwrap(),
            "a\nB\nc\n"
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
