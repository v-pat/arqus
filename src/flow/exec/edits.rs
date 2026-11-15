//! Edit format parser and atomic application.

use std::collections::HashMap;
use std::path::{Component, Path};

use crate::edit_match::{apply_exact_edit, EditMatchError};
use crate::flow::errors::FlowError;
use crate::safety::{check_path, PathOp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditBlock {
    Search {
        path: String,
        search: String,
        replace: String,
    },
    File {
        path: String,
        content: String,
    },
}

impl EditBlock {
    pub fn path(&self) -> &str {
        match self {
            Self::Search { path, .. } | Self::File { path, .. } => path,
        }
    }
}

enum ParseState {
    Outside,
    InSearchSearch {
        path: String,
        search_lines: Vec<String>,
    },
    InSearchReplace {
        path: String,
        search_lines: Vec<String>,
        replace_lines: Vec<String>,
    },
    InFile {
        path: String,
        file_lines: Vec<String>,
    },
}

pub fn parse_edits(output: &str) -> Result<Vec<EditBlock>, FlowError> {
    let trimmed = output.trim();
    if let Some(stripped) = trimmed.strip_prefix("CANNOT_COMPLETE:") {
        let reason = stripped.trim();
        return Err(FlowError::Declined(reason.to_string()));
    }
    if trimmed.is_empty() {
        return Err(FlowError::NoOutput);
    }

    let mut edits = Vec::new();
    let mut state = ParseState::Outside;

    for line in output.lines() {
        match state {
            ParseState::Outside => {
                if let Some(rest) = line.strip_prefix("<<<<<<< SEARCH") {
                    let path = rest.trim();
                    if path.is_empty() {
                        return Err(FlowError::ParseError("missing path in SEARCH".into()));
                    }
                    state = ParseState::InSearchSearch {
                        path: path.to_string(),
                        search_lines: Vec::new(),
                    };
                } else if let Some(rest) = line.strip_prefix("<<<<<<< FILE") {
                    let path = rest.trim();
                    if path.is_empty() {
                        return Err(FlowError::ParseError("missing path in FILE".into()));
                    }
                    state = ParseState::InFile {
                        path: path.to_string(),
                        file_lines: Vec::new(),
                    };
                }
            }
            ParseState::InSearchSearch {
                path,
                mut search_lines,
            } => {
                if line == "=======" {
                    state = ParseState::InSearchReplace {
                        path,
                        search_lines,
                        replace_lines: Vec::new(),
                    };
                } else if line.starts_with("<<<<<<< SEARCH") || line.starts_with("<<<<<<< FILE") {
                    return Err(FlowError::ParseError("unclosed SEARCH block".into()));
                } else {
                    search_lines.push(line.to_string());
                    state = ParseState::InSearchSearch { path, search_lines };
                }
            }
            ParseState::InSearchReplace {
                path,
                search_lines,
                mut replace_lines,
            } => {
                if line.starts_with(">>>>>>> REPLACE") {
                    edits.push(EditBlock::Search {
                        path,
                        search: search_lines.join("\n"),
                        replace: replace_lines.join("\n"),
                    });
                    state = ParseState::Outside;
                } else if line.starts_with("<<<<<<< SEARCH") || line.starts_with("<<<<<<< FILE") {
                    return Err(FlowError::ParseError("unclosed SEARCH block".into()));
                } else {
                    replace_lines.push(line.to_string());
                    state = ParseState::InSearchReplace {
                        path,
                        search_lines,
                        replace_lines,
                    };
                }
            }
            ParseState::InFile {
                path,
                mut file_lines,
            } => {
                if line.starts_with(">>>>>>> END") {
                    let content = if file_lines.is_empty() {
                        String::new()
                    } else {
                        let mut c = file_lines.join("\n");
                        c.push('\n');
                        c
                    };
                    edits.push(EditBlock::File { path, content });
                    state = ParseState::Outside;
                } else if line.starts_with("<<<<<<< FILE") {
                    return Err(FlowError::ParseError("unclosed FILE block".into()));
                } else {
                    file_lines.push(line.to_string());
                    state = ParseState::InFile { path, file_lines };
                }
            }
        }
    }

    match state {
        ParseState::Outside => {}
        _ => return Err(FlowError::ParseError("unclosed block at EOF".into())),
    }

    if edits.is_empty() {
        return Err(FlowError::NoOutput);
    }
    Ok(edits)
}

fn path_allowed(path: &str, allowed_writes: &[String]) -> bool {
    for allowed in allowed_writes {
        if allowed == path {
            return true;
        }
        if let Some(prefix) = allowed.strip_suffix("/**") {
            let dir_prefix = format!("{prefix}/");
            if path.starts_with(&dir_prefix) {
                return true;
            }
        }
    }
    false
}

pub fn apply_edits(
    edits: &[EditBlock],
    repo_root: &Path,
    allowed_writes: &[String],
) -> Result<Vec<String>, FlowError> {
    for edit in edits {
        let p = edit.path();
        if p.starts_with('/') || Path::new(p).is_absolute() {
            return Err(FlowError::ScopeViolation(format!("absolute path '{p}'")));
        }
        if Path::new(p)
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(FlowError::ScopeViolation(format!(
                "path '{p}' contains '..'"
            )));
        }
        if !path_allowed(p, allowed_writes) {
            return Err(FlowError::ScopeViolation(format!(
                "path '{p}' outside allowed_writes"
            )));
        }
        let arqus_home = repo_root.join(".arqus");
        check_path(repo_root, p, PathOp::Write, &[], &arqus_home, None)
            .map_err(FlowError::ScopeViolation)?;
    }

    let mut in_memory: HashMap<String, String> = HashMap::new();
    let mut order = Vec::new();

    for edit in edits {
        let p = edit.path().to_string();
        match edit {
            EditBlock::File { content, .. } => {
                if !in_memory.contains_key(&p) {
                    order.push(p.clone());
                }
                in_memory.insert(p, content.clone());
            }
            EditBlock::Search {
                search, replace, ..
            } => {
                let current = match in_memory.get(&p) {
                    Some(c) => c.clone(),
                    None => {
                        let full = repo_root.join(&p);
                        std::fs::read_to_string(&full).map_err(|err| {
                            FlowError::NoMatch(format!("could not read '{p}': {err}"))
                        })?
                    }
                };
                let updated = match apply_exact_edit(&current, search, replace) {
                    Ok(u) => u,
                    Err(EditMatchError::ZeroMatches { line_count }) => {
                        return Err(FlowError::NoMatch(format!(
                            "search block not found in {p} ({line_count} lines). Check indentation and whitespace."
                        )));
                    }
                    Err(EditMatchError::MultipleMatches { count }) => {
                        return Err(FlowError::AmbiguousMatch(format!(
                            "search block found {count} times in {p}; add more surrounding lines to make it unique"
                        )));
                    }
                    Err(EditMatchError::EmptyOldText) => {
                        return Err(FlowError::ParseError(
                            "search text must not be empty".into(),
                        ));
                    }
                };
                if !in_memory.contains_key(&p) {
                    order.push(p.clone());
                }
                in_memory.insert(p, updated);
            }
        }
    }

    for p in &order {
        if let Some(content) = in_memory.get(p) {
            let full = repo_root.join(p);
            crate::atomic_write::write_atomic(&full, content.as_bytes())
                .map_err(|err| FlowError::Git(format!("failed to write {p}: {err}")))?;
        }
    }

    Ok(order)
}

#[cfg(test)]
#[path = "edits_tests.rs"]
mod tests;
