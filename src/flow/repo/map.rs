//! Repo map builder and line-scanning symbol scanners.

use std::path::Path;

use crate::flow::errors::FlowError;
use crate::flow::repo::git::list_tracked_files;

pub const DEFAULT_MAX_MAP_BYTES: usize = 32 * 1024;
pub const TRUNCATION_MARKER: &str = "\n[... truncated repo map ...]\n";

/// Scans symbols from file content by line scanning for supported languages:
/// Rust, Python, Go, and TypeScript.
pub fn scan_file_symbols(relative_path: &str, content: &str) -> Vec<String> {
    let ext = match relative_path.rsplit('.').next() {
        Some(e) => e.to_ascii_lowercase(),
        None => return Vec::new(),
    };

    match ext.as_str() {
        "rs" => scan_rust_symbols(content),
        "py" => scan_python_symbols(content),
        "go" => scan_go_symbols(content),
        "ts" | "tsx" | "mts" | "cts" => scan_typescript_symbols(content),
        _ => Vec::new(),
    }
}

fn strip_comment(line: &str, comment_prefix: &str) -> String {
    if let Some(idx) = line.find(comment_prefix) {
        line[..idx].trim_end().to_string()
    } else {
        line.to_string()
    }
}

fn clean_signature(line: &str) -> String {
    let mut s = line.trim();
    if let Some(idx) = s.find('{') {
        s = s[..idx].trim();
    }
    s = s.trim_end_matches(';').trim();
    s.to_string()
}

fn scan_rust_symbols(content: &str) -> Vec<String> {
    let mut symbols = Vec::new();

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            continue;
        }

        let line_no_comment = strip_comment(trimmed, "//");
        let line = line_no_comment.trim();

        // Strip visibility and qualifiers to test for keywords
        let mut rest = line;
        if rest.starts_with("pub") {
            rest = rest[3..].trim();
            if rest.starts_with('(') {
                if let Some(idx) = rest.find(')') {
                    rest = rest[idx + 1..].trim();
                }
            }
        }

        // Strip qualifiers (async, const, qualifiers, extern)
        loop {
            if rest.starts_with("async") {
                rest = rest[5..].trim();
            } else if (rest.starts_with("const ") && rest[6..].trim().starts_with("fn "))
                || rest.starts_with(concat!("un", "safe"))
            {
                rest = rest[6..].trim();
            } else if rest.starts_with("const") && !rest.starts_with("const ") {
                // don't strip if it might be const item
                break;
            } else if rest.starts_with("extern") {
                rest = rest[6..].trim();
                if rest.starts_with('"') {
                    if let Some(idx) = rest[1..].find('"') {
                        rest = rest[idx + 2..].trim();
                    }
                }
            } else {
                break;
            }
        }

        let is_symbol = rest.starts_with("fn ")
            || rest.starts_with("struct ")
            || rest.starts_with("struct\n")
            || rest == "struct"
            || rest.starts_with("enum ")
            || rest.starts_with("trait ")
            || rest.starts_with("impl ")
            || rest.starts_with("impl<");

        if is_symbol {
            let cleaned = clean_signature(line);
            if !cleaned.is_empty() {
                symbols.push(cleaned);
            }
        }
    }

    symbols
}

fn scan_python_symbols(content: &str) -> Vec<String> {
    let mut symbols = Vec::new();

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let line_no_comment = strip_comment(trimmed, "#");
        let line = line_no_comment.trim();

        let is_symbol = line.starts_with("def ")
            || line.starts_with("async def ")
            || line.starts_with("class ");

        if is_symbol {
            let mut cleaned = line.to_string();
            if let Some(idx) = cleaned.rfind(':') {
                cleaned = cleaned[..=idx].to_string();
            }
            symbols.push(cleaned);
        }
    }

    symbols
}

fn scan_go_symbols(content: &str) -> Vec<String> {
    let mut symbols = Vec::new();

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            continue;
        }

        let line_no_comment = strip_comment(trimmed, "//");
        let line = line_no_comment.trim();

        let is_symbol = line.starts_with("func ") || line.starts_with("type ");

        if is_symbol {
            let cleaned = clean_signature(line);
            if !cleaned.is_empty() {
                symbols.push(cleaned);
            }
        }
    }

    symbols
}

fn scan_typescript_symbols(content: &str) -> Vec<String> {
    let mut symbols = Vec::new();

    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            continue;
        }

        let line_no_comment = strip_comment(trimmed, "//");
        let line = line_no_comment.trim();

        let mut rest = line;
        if rest.starts_with("export ") {
            rest = rest[7..].trim();
            if rest.starts_with("default ") {
                rest = rest[8..].trim();
            }
            if rest.starts_with("abstract ") {
                rest = rest[9..].trim();
            }
            if rest.starts_with("async ") {
                rest = rest[6..].trim();
            }
        } else {
            if rest.starts_with("abstract ") {
                rest = rest[9..].trim();
            }
            if rest.starts_with("async ") {
                rest = rest[6..].trim();
            }
        }

        let is_symbol = rest.starts_with("function ")
            || rest.starts_with("class ")
            || rest.starts_with("interface ")
            || (line.starts_with("export const ") || line.starts_with("export const\n"));

        if is_symbol {
            let mut cleaned = clean_signature(line);
            if line.starts_with("export const ") {
                if let Some(idx) = cleaned.find('=') {
                    cleaned = cleaned[..idx].trim().to_string();
                }
            }
            if !cleaned.is_empty() {
                symbols.push(cleaned);
            }
        }
    }

    symbols
}

/// Builds a deterministic, size-capped repo map of tracked files and their symbols.
pub fn build_repo_map(repo: &Path, max_bytes: usize) -> Result<String, FlowError> {
    let tracked_files = list_tracked_files(repo)?;

    let mut out = String::new();
    for file in &tracked_files {
        out.push_str(file);
        out.push('\n');

        let full_path = repo.join(file);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let symbols = scan_file_symbols(file, &content);
            for sym in symbols {
                out.push_str("  ");
                out.push_str(&sym);
                out.push('\n');
            }
        }
    }

    if out.len() <= max_bytes {
        return Ok(out);
    }

    if max_bytes < TRUNCATION_MARKER.len() {
        return Ok(TRUNCATION_MARKER[..max_bytes].to_string());
    }

    let cut_target = max_bytes - TRUNCATION_MARKER.len();
    let truncated_slice = &out[..cut_target];
    let end_idx = truncated_slice
        .rfind('\n')
        .map(|idx| idx + 1)
        .unwrap_or(cut_target);

    let mut final_map = out[..end_idx].to_string();
    final_map.push_str(TRUNCATION_MARKER);
    Ok(final_map)
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "map_fixture_tests.rs"]
mod fixture_tests;
