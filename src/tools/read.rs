//! The `read` tool.

use std::path::Path;

use serde_json::Value;

use super::{arg_str, arg_usize, resolve_path, truncate_bytes, ToolOutput};

/// Max bytes returned by one read call.
pub const MAX_BYTES: usize = 50_000;
/// Default max lines returned by one read call.
pub const MAX_LINES: usize = 2_000;
/// Lines longer than this are cut, so one minified file cannot flood the context.
pub const MAX_LINE_CHARS: usize = 2_000;
/// A NUL byte in this prefix means the file is binary.
pub const BINARY_SNIFF_BYTES: usize = 8_000;

pub fn run(args: &Value, cwd: &Path) -> ToolOutput {
    let path = match arg_str(args, "path") {
        Ok(path) => path,
        Err(err) => return ToolOutput::error(err),
    };
    let offset = match arg_usize(args, "offset", 1) {
        Ok(offset) => offset,
        Err(err) => return ToolOutput::error(err),
    };
    let limit = match arg_usize(args, "limit", MAX_LINES) {
        Ok(limit) => limit,
        Err(err) => return ToolOutput::error(err),
    };

    let full = resolve_path(cwd, &path);
    let meta = match std::fs::metadata(&full) {
        Ok(meta) => meta,
        Err(err) => return ToolOutput::error(format!("could not read {}: {err}", full.display())),
    };

    if meta.is_dir() {
        return read_dir_listing(&full, offset, limit);
    }

    let bytes = match std::fs::read(&full) {
        Ok(bytes) => bytes,
        Err(err) => return ToolOutput::error(format!("could not read {}: {err}", full.display())),
    };

    let sniff = &bytes[..bytes.len().min(BINARY_SNIFF_BYTES)];
    if sniff.contains(&0) {
        return ToolOutput::error(format!("binary file: {path}"));
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return ToolOutput::error(format!("{} is not valid UTF-8 text", full.display())),
    };

    slice_text(&text, offset, limit)
}

/// A directory path returns a sorted listing, with a trailing `/` on folders.
fn read_dir_listing(dir: &Path, offset: usize, limit: usize) -> ToolOutput {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            return ToolOutput::error(format!("could not list {}: {err}", dir.display()));
        }
    };

    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let mut name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        if is_dir {
            name.push('/');
        }
        names.push(name);
    }
    names.sort();

    if names.is_empty() {
        return ToolOutput::ok(format!("{} is an empty directory", dir.display()));
    }
    slice_text(&names.join("\n"), offset, limit)
}

/// Apply the line window and the byte cap. Pure, so it is easy to test.
fn slice_text(text: &str, offset: usize, limit: usize) -> ToolOutput {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return ToolOutput::ok(String::new());
    }
    let start = offset.max(1) - 1;
    if start >= lines.len() {
        return ToolOutput::error(format!(
            "offset {offset} is past the end of the file ({} lines)",
            lines.len()
        ));
    }
    let end = start.saturating_add(limit.max(1)).min(lines.len());

    // Cut very long lines first, on character boundaries.
    let window: Vec<String> = lines[start..end]
        .iter()
        .map(|line| cut_line(line))
        .collect();

    let mut out = String::new();
    let mut next = start;
    let mut partial = false;

    for (i, line) in window.iter().enumerate() {
        let added = if out.is_empty() {
            line.len()
        } else {
            1 + line.len()
        };
        if out.len() + added > MAX_BYTES {
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        next = start + i + 1;
    }

    if out.is_empty() {
        // One line longer than the whole budget: return its head, skip the line.
        let line = &window[0];
        out.push_str(truncate_bytes(line, MAX_BYTES));
        next = start + 1;
        partial = true;
    }

    if next == lines.len() && text.ends_with('\n') && !out.is_empty() && !partial {
        out.push('\n');
    }

    if next < lines.len() || partial {
        out.push_str(&format!(
            "\n[truncated: call read again with offset={} to continue]",
            next + 1
        ));
    }

    ToolOutput::ok(out)
}

/// Cut one line to `MAX_LINE_CHARS` characters and say so.
fn cut_line(line: &str) -> String {
    if line.chars().count() <= MAX_LINE_CHARS {
        return line.to_string();
    }
    let head: String = line.chars().take(MAX_LINE_CHARS).collect();
    format!("{head}...[line truncated]")
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
