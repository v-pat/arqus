use crate::types::*;
use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
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

/// `~/.pirs/sessions/<slug>/` for the given working directory.
pub fn sessions_dir(cwd: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".pirs").join("sessions").join(project_slug(cwd))
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
        let reader = BufReader::new(File::open(path).context("cannot open session file")?);
        let mut lines = reader.lines();
        let first = lines.next().context("empty session file")??;
        let header: Header = serde_json::from_str(&first).context("bad header line")?;
        let mut entries = Vec::new();
        for line in lines {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Entry>(&line) {
                Ok(e) => entries.push(e),
                Err(_) => continue, // a half-written last line after a crash: skip it
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

/// Walk from `leaf` up to the root. Returns entries ordered root -> leaf.
pub fn path_to_root<'a>(entries: &'a [Entry], leaf: Option<&str>) -> Vec<&'a Entry> {
    let mut out: Vec<&Entry> = Vec::new();
    let mut cur = leaf.map(|s| s.to_string());
    while let Some(id) = cur {
        match entries.iter().find(|e| e.id == id) {
            Some(e) => {
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
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!("pirs_session_{}", new_id()))
    }

    fn user(t: &str) -> EntryKind {
        EntryKind::Message {
            message: Message::User {
                text: t.to_string(),
            },
        }
    }

    fn texts(entries: &[Entry], leaf: Option<&str>) -> Vec<String> {
        path_to_root(entries, leaf)
            .into_iter()
            .map(|e| match &e.kind {
                EntryKind::Message {
                    message: Message::User { text },
                } => text.clone(),
                _ => "?".to_string(),
            })
            .collect()
    }

    #[test]
    fn reopen_keeps_entries_and_leaf() {
        let dir = tmp();
        let mut s = Session::create(&dir, "/x").unwrap();
        s.append(user("a")).unwrap();
        s.append(user("b")).unwrap();
        let s2 = Session::open(&s.path).unwrap();
        assert_eq!(s2.entries, s.entries);
        assert_eq!(s2.leaf_id, s.leaf_id);
    }

    #[test]
    fn broken_last_line_is_skipped() {
        let dir = tmp();
        let mut s = Session::create(&dir, "/x").unwrap();
        s.append(user("a")).unwrap();
        let mut f = OpenOptions::new().append(true).open(&s.path).unwrap();
        write!(f, "{{\"id\":\"zz\",\"par").unwrap();
        let s2 = Session::open(&s.path).unwrap();
        assert_eq!(s2.entries.len(), 1);
    }

    #[test]
    fn branch_moves_leaf() {
        let dir = tmp();
        let mut s = Session::create(&dir, "/x").unwrap();
        let a = s.append(user("a")).unwrap();
        s.append(user("b")).unwrap();
        s.set_leaf(&a).unwrap();
        s.append(user("c")).unwrap();
        assert_eq!(texts(&s.entries, s.leaf_id.as_deref()), vec!["a", "c"]);
    }

    #[test]
    fn reopened_leaf_is_last_line() {
        let dir = tmp();
        let mut s = Session::create(&dir, "/x").unwrap();
        s.append(user("a")).unwrap();
        let b = s.append(user("b")).unwrap();
        let s2 = Session::open(&s.path).unwrap();
        assert_eq!(s2.leaf_id.as_deref(), Some(b.as_str()));
    }

    #[test]
    fn slug_replaces_slash_and_dot() {
        assert_eq!(project_slug("/home/me/my.proj"), "-home-me-my-proj");
    }

    #[test]
    fn latest_session_picks_newest() {
        let dir = tmp();
        fs::create_dir_all(&dir).unwrap();
        let old = dir.join("100_a.jsonl");
        let new = dir.join("200_b.jsonl");
        fs::write(&old, "").unwrap();
        fs::write(&new, "").unwrap();
        assert_eq!(latest_session(&dir), Some(new));
    }

    #[test]
    fn list_sessions_is_newest_first() {
        let dir = tmp();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("100_a.jsonl"), "").unwrap();
        fs::write(dir.join("300_c.jsonl"), "").unwrap();
        fs::write(dir.join("200_b.jsonl"), "").unwrap();
        fs::write(dir.join("notes.txt"), "").unwrap();
        let names: Vec<String> = list_sessions(&dir)
            .into_iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        assert_eq!(names, vec!["300_c.jsonl", "200_b.jsonl", "100_a.jsonl"]);
    }

    #[test]
    fn sessions_dir_ends_with_slug() {
        let p = sessions_dir("/home/me/my.proj");
        assert!(p.ends_with(".pirs/sessions/-home-me-my-proj"));
    }
}
