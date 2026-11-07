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

    if old_text.is_empty() {
        return ToolOutput::error("old_text must not be empty");
    }
    if old_text == new_text {
        return ToolOutput::error("old_text and new_text are the same; nothing to edit");
    }

    let full = resolve_path(cwd, &path);
    let content = match std::fs::read_to_string(&full) {
        Ok(content) => content,
        Err(err) => return ToolOutput::error(format!("could not read {}: {err}", full.display())),
    };

    // A file with CRLF endings is matched as if it used LF, then written back
    // with the endings it already had.
    let crlf = content.contains("\r\n");
    let haystack = if crlf {
        content.replace("\r\n", "\n")
    } else {
        content.clone()
    };
    let old_norm = normalize(&old_text);
    let new_norm = normalize(&new_text);

    let count = haystack.matches(&old_norm).count();
    if count == 0 {
        return ToolOutput::error(format!(
            "old_text was not found in {path} ({} lines). Check the exact indentation and whether the file uses tabs or spaces.",
            haystack.lines().count()
        ));
    }
    if count > 1 {
        return ToolOutput::error(format!(
            "old_text was found {count} times in {path}; add more surrounding lines to make it unique"
        ));
    }

    let updated = haystack.replacen(&old_norm, &new_norm, 1);
    let updated = if crlf {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };

    if let Err(err) = std::fs::write(&full, updated) {
        return ToolOutput::error(format!("could not write {}: {err}", full.display()));
    }

    ToolOutput::ok(format!("Edited {path}"))
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
