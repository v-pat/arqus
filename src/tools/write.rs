//! The `write` tool: atomic replace through a temp file in the same folder.

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
    if full.is_dir() {
        return ToolOutput::error(format!("{path} is a directory"));
    }

    if let Err(err) = crate::atomic_write::write_atomic(&full, content.as_bytes()) {
        return ToolOutput::error(format!("could not write {}: {err}", full.display()));
    }

    ToolOutput::ok(format!("Wrote {} bytes to {path}", content.len()))
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
