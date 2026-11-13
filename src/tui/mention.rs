//! `@file` mentions: token detection, file matching and expansion.
//!
//! Everything here is pure: the editor hands over the visible text and the
//! cwd, and gets back plain strings.

use std::path::Path;

/// Files under 8 KB are inlined; each inline block is capped at 20 KB total.
pub const MAX_INLINE_BYTES: u64 = 8 * 1024;
pub const MAX_INLINE_FILES: usize = 3;
pub const MAX_INLINE_TOTAL: u64 = 20 * 1024;

/// The `@` word around the cursor: `(char index of '@', query after it)`.
///
/// A word ends at whitespace, so `@src/a.rs` is one token.
pub fn token_at(chars: &[char], cursor: usize) -> Option<(usize, String)> {
    let cursor = cursor.min(chars.len());
    let mut start = cursor;
    while start > 0 && !chars[start - 1].is_whitespace() {
        start -= 1;
    }
    let word: String = chars[start..cursor].iter().collect();
    let query = word.strip_prefix('@')?;
    Some((start, query.to_string()))
}

/// Char index just past the `@` word that starts at `start`.
pub fn token_end(chars: &[char], start: usize) -> usize {
    let mut end = start.min(chars.len());
    while end < chars.len() && !chars[end].is_whitespace() {
        end += 1;
    }
    end
}

/// Best matches for a query, prefix hits first, capped at `limit`.
pub fn files_for(files: &[String], query: &str, limit: usize) -> Vec<String> {
    let needle = query.to_ascii_lowercase();
    let mut ranked: Vec<(u8, usize, &String)> = files
        .iter()
        .enumerate()
        .filter_map(|(position, path)| {
            let lower = path.to_ascii_lowercase();
            if needle.is_empty() {
                Some((1, position, path))
            } else if lower.starts_with(&needle) || file_name(path).starts_with(&needle) {
                Some((0, position, path))
            } else if lower.contains(&needle) {
                Some((1, position, path))
            } else {
                None
            }
        })
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, _, path)| path.clone())
        .collect()
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase()
}

/// Every `@path` word in the text, duplicates removed, in first-seen order.
pub fn mentioned(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let Some(path) = word.strip_prefix('@') else {
            continue;
        };
        let path = path.trim_end_matches([',', '.', ';', ':', ')', ']', '"', '\'']);
        if path.is_empty() {
            continue;
        }
        if !out.iter().any(|seen| seen == path) {
            out.push(path.to_string());
        }
    }
    out
}

/// Append a block per mentioned file to the text sent to the model.
///
/// Small files are inlined (`<file path="...">`); larger files get one line
/// that asks the model to use the `read` tool instead.
pub fn expand(text: &str, cwd: &Path) -> String {
    let files = mentioned(text);
    if files.is_empty() {
        return text.to_string();
    }
    let root = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let mut out = text.to_string();
    let mut inline_bytes = 0u64;
    let mut inline_files = 0usize;
    for path in files {
        let full = cwd.join(&path);
        let resolved = full.canonicalize().ok();
        let inside = resolved
            .as_ref()
            .map(|real| real.starts_with(&root))
            .unwrap_or(false);
        let contents = if inside {
            std::fs::read_to_string(&full).ok()
        } else {
            None
        };
        let Some(contents) = contents else {
            out.push_str(&format!(
                "\n\n(file mentioned: {path}; outside the project or not readable, use the read tool)"
            ));
            continue;
        };
        let bytes = contents.len() as u64;
        let room = inline_files < MAX_INLINE_FILES
            && bytes <= MAX_INLINE_BYTES
            && inline_bytes + bytes <= MAX_INLINE_TOTAL;
        if room {
            inline_bytes += bytes;
            inline_files += 1;
            out.push_str(&format!("\n\n<file path=\"{path}\">\n{contents}\n</file>"));
        } else {
            let lines = contents.lines().count();
            out.push_str(&format!(
                "\n\n(file mentioned: {path}, {lines} lines; use the read tool)"
            ));
        }
    }
    out
}

#[cfg(test)]
#[path = "mention_tests.rs"]
mod tests;
