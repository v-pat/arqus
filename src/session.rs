use crate::types::*;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

/// "/home/me/my.project" -> "-home-me-my-project"
pub fn project_slug(cwd: &str) -> String {
    cwd.replace(['/', '.'], "-")
}

/// `<ARQUS_HOME>/sessions/<slug>/` for the given working directory.
pub fn sessions_dir(cwd: &str) -> PathBuf {
    crate::home::arqus_home()
        .join("sessions")
        .join(project_slug(cwd))
}

/// Every `.jsonl` session file in `dir`, newest first.
/// File names start with the timestamp in milliseconds, so sorting by name is enough.
pub fn list_sessions(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    files.sort();
    files.reverse();
    files
}

/// Newest `.jsonl` file in `dir`, for "continue last session".
pub fn latest_session(dir: &Path) -> Option<PathBuf> {
    list_sessions(dir).into_iter().next()
}

pub struct Session {
    pub path: PathBuf,
    pub header: Header,
    pub entries: Vec<Entry>,
    pub leaf_id: Option<String>,
}

impl Session {
    pub fn create(dir: &Path, cwd: &str) -> Result<Session> {
        fs::create_dir_all(dir)?;
        let header = Header {
            kind: "session".to_string(),
            version: 1,
            id: new_id(),
            cwd: cwd.to_string(),
            ts: now_ms(),
        };
        let path = dir.join(format!("{}_{}.jsonl", header.ts, header.id));
        let mut f = File::create(&path)?;
        writeln!(f, "{}", serde_json::to_string(&header)?)?;
        Ok(Session {
            path,
            header,
            entries: vec![],
            leaf_id: None,
        })
    }

    pub fn open(path: &Path) -> Result<Session> {
        let bytes = fs::read(path).context("cannot open session file")?;
        let text = String::from_utf8_lossy(&bytes);
        let mut lines: Vec<&str> = text.lines().collect();
        let first = lines.first().copied().context("empty session file")?;
        let header: Header = serde_json::from_str(first).context("bad header line")?;
        lines.remove(0);
        let last = lines.len().saturating_sub(1);
        let mut entries = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Entry>(line) {
                Ok(e) => entries.push(e),
                Err(err) => {
                    // A half-written last line after a crash is expected and silent.
                    // A bad line in the middle means real corruption: warn and skip.
                    if i != last {
                        eprintln!(
                            "warning: skipping corrupt line {} in {}: {err}",
                            i + 2,
                            path.display()
                        );
                    }
                }
            }
        }
        let leaf_id = entries.last().map(|e| e.id.clone());
        Ok(Session {
            path: path.to_path_buf(),
            header,
            entries,
            leaf_id,
        })
    }

    /// Add one entry as a child of the current leaf. Returns the new id.
    pub fn append(&mut self, kind: EntryKind) -> Result<String> {
        let entry = Entry {
            id: new_id(),
            parent_id: self.leaf_id.clone(),
            ts: now_ms(),
            kind,
        };
        let id = entry.id.clone();
        self.append_entry(entry)?;
        Ok(id)
    }

    /// Append an already-built entry, keeping its id, parent and timestamp.
    /// Used by `/fork`, which copies a path into a new file.
    pub fn append_entry(&mut self, entry: Entry) -> Result<()> {
        let mut f = OpenOptions::new().append(true).open(&self.path)?;
        writeln!(f, "{}", serde_json::to_string(&entry)?)?;
        self.leaf_id = Some(entry.id.clone());
        self.entries.push(entry);
        Ok(())
    }

    /// Branching = move the leaf pointer. The next append becomes a child of `id`.
    pub fn set_leaf(&mut self, id: &str) -> Result<()> {
        if !self.entries.iter().any(|e| e.id == id) {
            anyhow::bail!("no entry with id {id}");
        }
        self.leaf_id = Some(id.to_string());
        Ok(())
    }
}

/// The title shown for a session: the last `session_name` custom entry, else
/// the first user message. Never longer than 60 characters.
pub fn title(entries: &[Entry]) -> String {
    let mut name = None;
    let mut first_user = None;
    for entry in entries {
        match &entry.kind {
            EntryKind::Custom { custom_type, data } if custom_type == "session_name" => {
                if let Some(text) = data.get("name").and_then(|value| value.as_str()) {
                    if !text.trim().is_empty() {
                        name = Some(text.to_string());
                    }
                }
            }
            EntryKind::Message {
                message: crate::types::Message::User { text },
            } if first_user.is_none() => {
                first_user = Some(text.clone());
            }
            _ => {}
        }
    }
    flatten(&name.or(first_user).unwrap_or_default())
}

fn flatten(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(60).collect()
}

/// Walk from `leaf` up to the root. Returns entries ordered root -> leaf.
/// One id-to-index map is built per call, so a long chain costs O(n).
pub fn path_to_root<'a>(entries: &'a [Entry], leaf: Option<&str>) -> Vec<&'a Entry> {
    let index: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut out: Vec<&Entry> = Vec::new();
    let mut cur = leaf.map(|s| s.to_string());
    while let Some(id) = cur {
        match index.get(id.as_str()) {
            Some(&i) => {
                let e = &entries[i];
                out.push(e);
                cur = e.parent_id.clone();
            }
            None => break,
        }
    }
    out.reverse();
    out
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
