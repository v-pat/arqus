//! The `edit` tool: one unique exact replacement.

use std::path::Path;

use serde_json::Value;

use super::{arg_str, resolve_path, ToolOutput};
use crate::edit_match::{apply_exact_edit, EditMatchError};

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

    let updated = match apply_exact_edit(&content, &old_text, &new_text) {
        Ok(updated) => updated,
        Err(EditMatchError::EmptyOldText) => {
            return ToolOutput::error("old_text must not be empty");
        }
        Err(EditMatchError::ZeroMatches { line_count }) => {
            return ToolOutput::error(format!(
                "old_text was not found in {path} ({line_count} lines). Check the exact indentation and whether the file uses tabs or spaces."
            ));
        }
        Err(EditMatchError::MultipleMatches { count }) => {
            return ToolOutput::error(format!(
                "old_text was found {count} times in {path}; add more surrounding lines to make it unique"
            ));
        }
    };

    if let Err(err) = std::fs::write(&full, updated) {
        return ToolOutput::error(format!("could not write {}: {err}", full.display()));
    }

    ToolOutput::ok(format!("Edited {path}"))
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
