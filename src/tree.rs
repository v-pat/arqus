//! The session tree: indented rows for `/tree`, and `/fork` copying.

use std::path::Path;

use anyhow::Result;

use crate::session::{path_to_root, Session};
use crate::types::{Block, Entry, EntryKind, Message};

/// One line of the `/tree` view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub id: String,
    pub depth: usize,
    pub role: String,
    pub preview: String,
    pub on_path: bool,
}

/// Every entry in file order, with the number of ancestors as `depth`.
pub fn rows(entries: &[Entry], leaf: Option<&str>) -> Vec<TreeRow> {
    let on_path: Vec<String> = path_to_root(entries, leaf)
        .into_iter()
        .map(|e| e.id.clone())
        .collect();

    entries
        .iter()
        .map(|entry| {
            let (role, text) = describe(&entry.kind);
            TreeRow {
                id: entry.id.clone(),
                depth: depth(entries, entry),
                role,
                preview: preview(&text),
                on_path: on_path.iter().any(|id| id == &entry.id),
            }
        })
        .collect()
}

/// Number of ancestors of `entry` in the whole entry list.
fn depth(entries: &[Entry], entry: &Entry) -> usize {
    let mut count = 0;
    let mut parent = entry.parent_id.clone();
    // Guard against a corrupt file that points in a circle.
    while let Some(id) = parent {
        if count > entries.len() {
            break;
        }
        let Some(found) = entries.iter().find(|e| e.id == id) else {
            break;
        };
        count += 1;
        parent = found.parent_id.clone();
    }
    count
}

/// Short role name and the text shown in the tree.
fn describe(kind: &EntryKind) -> (String, String) {
    match kind {
        EntryKind::Message { message } => match message {
            Message::User { text } => ("user".to_string(), text.clone()),
            Message::Assistant { blocks, .. } => {
                let text: String = blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if !text.is_empty() {
                    return ("assistant".to_string(), text);
                }
                let calls: String = blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::ToolCall { name, .. } => Some(name.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                if !calls.is_empty() {
                    return ("assistant".to_string(), format!("tool_call: {calls}"));
                }
                let thinking: String = blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Thinking { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                ("assistant".to_string(), thinking)
            }
            Message::ToolResult { text, .. } => ("tool_result".to_string(), text.clone()),
        },
        EntryKind::ModelChange { provider, model } => {
            ("model_change".to_string(), format!("{provider}:{model}"))
        }
        EntryKind::Compaction { summary, .. } => ("compaction".to_string(), summary.clone()),
        EntryKind::BranchSummary { summary, .. } => ("branch_summary".to_string(), summary.clone()),
        EntryKind::Custom { custom_type, data } => {
            (format!("custom:{custom_type}"), data.to_string())
        }
    }
}

/// At most 60 characters, with line breaks flattened to single spaces.
fn preview(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(60).collect()
}

/// The current leaf path as readable ids, root first.
pub fn path_text(entries: &[Entry], leaf: Option<&str>) -> String {
    path_to_root(entries, leaf)
        .into_iter()
        .map(|e| e.id.clone())
        .collect::<Vec<_>>()
        .join(" > ")
}

/// Copy the entries on the current path into a new session file.
/// Ids and parents are kept, so the copy has the same history.
pub fn fork(session: &Session, dir: &Path) -> Result<Session> {
    let path = path_to_root(&session.entries, session.leaf_id.as_deref());
    let mut new_session = Session::create(dir, &session.header.cwd)?;
    for entry in path {
        new_session.append_entry(entry.clone())?;
    }
    Ok(new_session)
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
