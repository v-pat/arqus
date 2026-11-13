//! Fast session summaries for the `/sessions` picker.
//!
//! The file is read line by line so a 10,000-line session stays cheap: only the
//! lines that can carry a title are parsed as JSON.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::types::{Entry, EntryKind, Message};

use super::layout::relative_time;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub path: PathBuf,
    pub title: String,
    pub modified: u64,
    pub messages: usize,
}

impl SessionInfo {
    /// `2h ago · 14 msgs`, shown dim to the right of the title.
    pub fn detail(&self) -> String {
        format!("{} · {} msgs", relative_time(self.modified), self.messages)
    }
}

/// Read the title, modified time and entry count of one session file.
pub fn read(path: &Path) -> Option<SessionInfo> {
    let modified = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut messages = 0usize;
    let mut first_user = None;
    let mut name = None;
    let mut first_line = true;

    for line in reader.lines().map_while(Result::ok) {
        if first_line {
            first_line = false;
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        messages += 1;
        if line.contains("\"session_name\"") || (first_user.is_none() && line.contains("\"user\""))
        {
            if let Ok(Entry { kind, .. }) = serde_json::from_str::<Entry>(&line) {
                match kind {
                    EntryKind::Custom { custom_type, data } if custom_type == "session_name" => {
                        if let Some(text) = data.get("name").and_then(|value| value.as_str()) {
                            if !text.trim().is_empty() {
                                name = Some(text.to_string());
                            }
                        }
                    }
                    EntryKind::Message {
                        message: Message::User { text },
                    } if first_user.is_none() => {
                        first_user = Some(text);
                    }
                    _ => {}
                }
            }
        }
    }

    let title = name.or(first_user).unwrap_or_default();
    let title = flatten(&title);
    Some(SessionInfo {
        path: path.to_path_buf(),
        title: if title.is_empty() {
            "(empty session)".to_string()
        } else {
            title
        },
        modified,
        messages,
    })
}

fn flatten(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(60).collect()
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
