//! Exact-once text matching and replacement with CRLF preservation.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditMatchError {
    EmptyOldText,
    ZeroMatches { line_count: usize },
    MultipleMatches { count: usize },
}

/// Normalizes CRLF line endings to LF.
pub fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Matches `old_text` in `content` exactly once and replaces it with `new_text`.
///
/// If `content` uses CRLF line endings (`\r\n`), matching is performed as if
/// it used LF (`\n`), and the result is converted back to CRLF.
pub fn apply_exact_edit(
    content: &str,
    old_text: &str,
    new_text: &str,
) -> Result<String, EditMatchError> {
    if old_text.is_empty() {
        return Err(EditMatchError::EmptyOldText);
    }

    let crlf = content.contains("\r\n");
    let haystack = if crlf {
        content.replace("\r\n", "\n")
    } else {
        content.to_string()
    };
    let old_norm = normalize(old_text);
    let new_norm = normalize(new_text);

    let count = haystack.matches(&old_norm).count();
    if count == 0 {
        return Err(EditMatchError::ZeroMatches {
            line_count: haystack.lines().count(),
        });
    }
    if count > 1 {
        return Err(EditMatchError::MultipleMatches { count });
    }

    let updated = haystack.replacen(&old_norm, &new_norm, 1);
    let updated = if crlf {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };

    Ok(updated)
}

#[cfg(test)]
#[path = "edit_match_tests.rs"]
mod tests;
