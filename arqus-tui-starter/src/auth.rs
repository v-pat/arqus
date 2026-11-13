//! API keys saved by the TUI "Connect" dialog. File: <ARQUS_HOME>/auth.json, mode 0600.
//! Shape: {"openrouter": {"type":"api","key":"sk-..."}, "anthropic": {...}}
//! Rules: never print a key; never log a key; on a corrupt file do NOT overwrite it (back it up).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Credential {
    Api { key: String },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuthStore {
    pub providers: BTreeMap<String, Credential>,
}

impl AuthStore {
    /// Missing file = empty store. A file that cannot be parsed is an ERROR (the caller shows a warning).
    pub fn load(path: &Path) -> Result<AuthStore> {
        match fs::read_to_string(path) {
            Ok(text) => {
                if text.trim().is_empty() {
                    return Ok(AuthStore::default());
                }
                serde_json::from_str(&text).with_context(|| format!("{} is not valid auth data", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AuthStore::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    /// Move a corrupt file aside so a later save cannot destroy the user's data.
    pub fn back_up_corrupt(path: &Path) -> Option<PathBuf> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let bad = path.with_extension(format!("json.bad-{ts}"));
        fs::rename(path, &bad).ok().map(|_| bad)
    }

    /// Atomic save: write a temp file (mode 0600 from the first byte), then rename over the target.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
            }
        }
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(self)?;
        {
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(&tmp).with_context(|| format!("cannot write {}", tmp.display()))?;
            f.write_all(data.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn set_api_key(&mut self, provider: &str, key: &str) {
        self.providers.insert(provider.to_string(), Credential::Api { key: key.to_string() });
    }

    pub fn get_key(&self, provider: &str) -> Option<&str> {
        match self.providers.get(provider) {
            Some(Credential::Api { key }) if !key.is_empty() => Some(key.as_str()),
            _ => None,
        }
    }

    pub fn remove(&mut self, provider: &str) -> bool {
        self.providers.remove(provider).is_some()
    }
}

/// "sk-or-v1-abcdef1234567890" -> "sk-or-…7890". Never show more than 6 + 4 characters.
pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 14 {
        return "•".repeat(chars.len().min(12));
    }
    let head: String = chars[..6].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// Clean what a user pastes: spaces, newlines, quotes, "Bearer " prefix, zero-width characters.
pub fn clean_pasted_key(raw: &str) -> String {
    let mut s = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_string();
    if s.to_lowercase().starts_with("bearer ") {
        s = s[7..].to_string();
    }
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(*c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(label: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("arqus_auth_{label}_{}_{n}", std::process::id()))
    }

    #[test]
    fn roundtrip_and_remove() {
        let dir = tmp("rt");
        let path = dir.join("auth.json");
        let mut s = AuthStore::default();
        s.set_api_key("openrouter", "sk-or-v1-secret");
        s.set_api_key("anthropic", "sk-ant-x");
        s.save(&path).unwrap();
        let mut back = AuthStore::load(&path).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.get_key("openrouter"), Some("sk-or-v1-secret"));
        assert!(back.remove("openrouter"));
        assert!(!back.remove("openrouter"));
        assert_eq!(back.get_key("openrouter"), None);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"type\": \"api\""));
    }

    #[cfg(unix)]
    #[test]
    fn file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp("perm");
        let path = dir.join("auth.json");
        let mut s = AuthStore::default();
        s.set_api_key("x", "k");
        s.save(&path).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        // saving again keeps the mode
        s.save(&path).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(!dir.join("auth.json.tmp").exists());
    }

    #[test]
    fn missing_and_empty_files_are_empty_stores() {
        let dir = tmp("miss");
        assert_eq!(AuthStore::load(&dir.join("auth.json")).unwrap(), AuthStore::default());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("auth.json"), "  \n").unwrap();
        assert_eq!(AuthStore::load(&dir.join("auth.json")).unwrap(), AuthStore::default());
    }

    #[test]
    fn corrupt_file_is_an_error_and_can_be_backed_up() {
        let dir = tmp("bad");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(AuthStore::load(&path).is_err());
        let moved = AuthStore::back_up_corrupt(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(moved).unwrap(), "{ not json");
        assert_eq!(AuthStore::load(&path).unwrap(), AuthStore::default());
    }

    #[test]
    fn mask_never_leaks_the_middle() {
        let m = mask_key("sk-or-v1-abcdef1234567890");
        assert_eq!(m, "sk-or-…7890");
        assert!(!m.contains("abcdef"));
        assert_eq!(mask_key("short"), "•••••");
        assert_eq!(mask_key(""), "");
    }

    #[test]
    fn pasted_keys_are_cleaned() {
        assert_eq!(clean_pasted_key("  sk-or-v1-abc \n"), "sk-or-v1-abc");
        assert_eq!(clean_pasted_key("\"sk-abc\""), "sk-abc");
        assert_eq!(clean_pasted_key("Bearer sk-abc"), "sk-abc");
        assert_eq!(clean_pasted_key("sk-\u{200b}abc\r\n"), "sk-abc");
        assert_eq!(clean_pasted_key("   "), "");
    }
}
