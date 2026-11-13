//! The `write` tool: atomic replace through a temp file in the same folder.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use super::{arg_str, resolve_path, ToolOutput};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    if let Some(parent) = full.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            return ToolOutput::error(format!(
                "could not create folder {}: {err}",
                parent.display()
            ));
        }
    }

    // Keep the permissions of an existing file; a new file gets the default.
    let permissions = std::fs::metadata(&full)
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.permissions());

    let temp = temp_path(&full);
    if let Err(err) = std::fs::write(&temp, &content) {
        std::fs::remove_file(&temp).ok();
        return ToolOutput::error(format!("could not write {}: {err}", full.display()));
    }
    if let Some(permissions) = permissions {
        if let Err(err) = std::fs::set_permissions(&temp, permissions) {
            std::fs::remove_file(&temp).ok();
            return ToolOutput::error(format!(
                "could not keep permissions on {}: {err}",
                full.display()
            ));
        }
    }
    if let Err(err) = std::fs::rename(&temp, &full) {
        std::fs::remove_file(&temp).ok();
        return ToolOutput::error(format!("could not replace {}: {err}", full.display()));
    }

    ToolOutput::ok(format!("Wrote {} bytes to {path}", content.len()))
}

/// A unique temp file next to `full`, so the rename stays on one filesystem.
fn temp_path(full: &Path) -> PathBuf {
    let dir = full.parent().unwrap_or_else(|| Path::new("."));
    let name = full
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(".{name}.arqus-tmp-{}-{n}", std::process::id()))
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
