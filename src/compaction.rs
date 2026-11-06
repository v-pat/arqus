use std::collections::BTreeSet;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::ai::Llm;
use crate::session::{path_to_root, Session};
use crate::types::*;

#[path = "compaction_summary.rs"]
mod summary;
use summary::generate_summary;

pub fn estimate_tokens(m: &Message) -> u64 {
    let chars: usize = match m {
        Message::User { text } => text.len(),
        Message::ToolResult { text, .. } => text.len(),
        Message::Assistant { blocks, .. } => blocks
            .iter()
            .map(|b| match b {
                Block::Text { text } | Block::Thinking { text } => text.len(),
                Block::ToolCall { name, args, .. } => name.len() + args.to_string().len(),
            })
            .sum(),
    };
    (chars as u64) / 4
}

pub fn should_compact(context_tokens: u64, context_window: u64, reserve_tokens: u64) -> bool {
    context_tokens > context_window.saturating_sub(reserve_tokens)
}

/// `path` = entries root->leaf. Returns the index of the first entry to KEEP,
/// or None if nothing can be cut. Never returns a tool_result entry.
pub fn find_cut_point(path: &[&Entry], keep_recent_tokens: u64) -> Option<usize> {
    let mut acc: u64 = 0;
    let mut cut: Option<usize> = None;
    for i in (0..path.len()).rev() {
        if let EntryKind::Message { message } = &path[i].kind {
            acc += estimate_tokens(message);
            let valid = !matches!(message, Message::ToolResult { .. });
            if valid {
                cut = Some(i);
            }
            if acc >= keep_recent_tokens && valid {
                break;
            }
        }
    }
    match cut {
        Some(0) | None => None, // would summarize nothing
        c => c,
    }
}

/// Context size for the compaction check: the last real usage count when the
/// provider gave one, otherwise the `chars / 4` estimate over every message.
pub fn context_tokens(messages: &[Message]) -> u64 {
    for m in messages.iter().rev() {
        if let Message::Assistant {
            usage: Some(usage), ..
        } = m
        {
            return usage.total();
        }
    }
    messages.iter().map(estimate_tokens).sum()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: u64,
    pub keep_recent_tokens: u64,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        CompactionSettings {
            enabled: true,
            reserve_tokens: 16_384,
            keep_recent_tokens: 20_000,
        }
    }
}

/// Plain text transcript for the summarizer. Labels stop it from continuing
/// the conversation instead of describing it.
pub fn serialize_conversation(messages: &[Message]) -> String {
    let mut out = String::new();
    for m in messages {
        match m {
            Message::User { text } => {
                out.push_str(&format!("[User]: {text}\n"));
            }
            Message::Assistant { blocks, .. } => {
                for block in blocks {
                    match block {
                        Block::Text { text } => {
                            out.push_str(&format!("[Assistant]: {text}\n"));
                        }
                        Block::Thinking { text } => {
                            out.push_str(&format!("[Assistant thinking]: {text}\n"));
                        }
                        Block::ToolCall { name, args, .. } => {
                            out.push_str(&format!(
                                "[Assistant tool calls]: {name}({})\n",
                                format_args(args)
                            ));
                        }
                    }
                }
            }
            Message::ToolResult { text, .. } => {
                let short: String = text.chars().take(2000).collect();
                out.push_str(&format!("[Tool result]: {short}\n"));
            }
        }
    }
    out
}

fn format_args(args: &serde_json::Value) -> String {
    match args {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

/// Files read and modified according to `read`/`write`/`edit` tool calls.
/// Lists from an earlier compaction entry are carried forward.
/// Modified = written + edited. Read = read minus modified. Both sorted, unique.
pub fn extract_file_ops(entries: &[Entry]) -> (Vec<String>, Vec<String>) {
    let mut read: BTreeSet<String> = BTreeSet::new();
    let mut modified: BTreeSet<String> = BTreeSet::new();

    for e in entries {
        match &e.kind {
            EntryKind::Compaction {
                read_files,
                modified_files,
                ..
            } => {
                read.extend(read_files.iter().cloned());
                modified.extend(modified_files.iter().cloned());
            }
            EntryKind::Message {
                message: Message::Assistant { blocks, .. },
            } => {
                for block in blocks {
                    let Block::ToolCall { name, args, .. } = block else {
                        continue;
                    };
                    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    match name.as_str() {
                        "read" => {
                            read.insert(path.to_string());
                        }
                        "write" | "edit" => {
                            modified.insert(path.to_string());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let read_only = read.difference(&modified).cloned().collect();
    (read_only, modified.into_iter().collect())
}

/// Summarize everything between the previous compaction and the cut point.
/// Returns false when nothing can be cut.
///
/// TODO: if the cut lands in the middle of a very long turn, pi writes a
/// separate "turn prefix" summary. We skip that and summarize the partial
/// turn like any other messages.
pub async fn compact(
    session: &mut Session,
    llm: &Llm,
    cfg: &CompactionSettings,
    custom: Option<&str>,
) -> Result<bool> {
    if !cfg.enabled {
        return Ok(false);
    }

    let path = path_to_root(&session.entries, session.leaf_id.as_deref());
    let Some(cut) = find_cut_point(&path, cfg.keep_recent_tokens) else {
        return Ok(false);
    };

    // The region to summarize starts where the previous compaction's kept
    // region starts, or at the root when there is no earlier compaction.
    let start = path
        .iter()
        .rposition(|e| matches!(e.kind, EntryKind::Compaction { .. }))
        .map(|ci| match &path[ci].kind {
            EntryKind::Compaction { first_kept_id, .. } => path[..ci]
                .iter()
                .position(|e| &e.id == first_kept_id)
                .unwrap_or(ci),
            _ => 0,
        })
        .unwrap_or(0);

    if cut <= start {
        return Ok(false);
    }

    let to_summarize: Vec<Message> = path[start..cut]
        .iter()
        .filter_map(|e| entry_message(e))
        .cloned()
        .collect();
    if to_summarize.is_empty() {
        return Ok(false);
    }

    let all_messages: Vec<Message> = path
        .iter()
        .filter_map(|e| entry_message(e))
        .cloned()
        .collect();
    let tokens_before = context_tokens(&all_messages);
    let first_kept_id = path[cut].id.clone();
    let previous = path[..cut].iter().rev().find_map(|e| match &e.kind {
        EntryKind::Compaction { summary, .. } => Some(summary.clone()),
        _ => None,
    });
    let prefix: Vec<Entry> = path[..cut].iter().map(|e| (*e).clone()).collect();
    let (read_files, modified_files) = extract_file_ops(&prefix);
    let conversation = serialize_conversation(&to_summarize);

    let summary = generate_summary(llm, &conversation, previous.as_deref(), custom).await?;

    session.append(EntryKind::Compaction {
        summary,
        first_kept_id,
        tokens_before,
        read_files,
        modified_files,
    })?;
    Ok(true)
}

fn entry_message(e: &Entry) -> Option<&Message> {
    match &e.kind {
        EntryKind::Message { message } => Some(message),
        _ => None,
    }
}

#[cfg(test)]
#[path = "compaction_tests.rs"]
mod tests;
