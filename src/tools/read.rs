//! The `read` tool.

use std::path::Path;

use serde_json::Value;

use super::{arg_str, arg_usize, resolve_path, ToolOutput};

/// Max bytes returned by one read call.
pub const MAX_BYTES: usize = 50_000;
/// Default max lines returned by one read call.
pub const MAX_LINES: usize = 2_000;

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
    let bytes = match std::fs::read(&full) {
        Ok(bytes) => bytes,
        Err(err) => return ToolOutput::error(format!("could not read {}: {err}", full.display())),
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return ToolOutput::error(format!("{} is not valid UTF-8 text", full.display())),
    };

    slice_text(&text, offset, limit)
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

    let mut out = String::new();
    let mut next = start;
    let mut partial = false;

    for (i, line) in lines[start..end].iter().enumerate() {
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
        // One line longer than the byte cap: return its head, skip the line.
        let line = lines[start];
        let cut = floor_char_boundary(line, MAX_BYTES);
        out.push_str(&line[..cut]);
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

/// Largest char boundary at or below `max`.
fn floor_char_boundary(text: &str, max: usize) -> usize {
    if text.len() <= max {
        return text.len();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_by_line_and_reports_next_offset() {
        let text = "a\nb\nc\nd\n";
        let out = slice_text(text, 2, 2);
        assert!(out.text.starts_with("b\nc"));
        assert!(out.text.contains("offset=4"));
    }

    #[test]
    fn no_truncation_for_short_file() {
        let out = slice_text("a\nb\n", 1, 10);
        assert_eq!(out.text, "a\nb\n");
    }

    #[test]
    fn offset_past_end_is_error() {
        let out = slice_text("a\nb\n", 9, 10);
        assert!(out.is_error);
    }

    #[test]
    fn byte_cap_keeps_head() {
        let text = "x".repeat(MAX_BYTES + 10);
        let out = slice_text(&text, 1, 10);
        assert!(out.text.len() <= MAX_BYTES + 80);
        assert!(out.text.contains("truncated"));
    }
}
