//! Optional debug log at `<ARQUS_HOME>/debug.log`.
//!
//! Enabled by `--debug` or `ARQUS_DEBUG=1`. One JSON object per line, so the
//! file can be read with `jq` or any JSON-lines tool. Headers are never
//! logged, and every line goes through `redact` before it is written.
//!
//! Rust note: `OnceLock` is a global slot that is written once and then only
//! read. That lets `log` be called from anywhere without passing a logger.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

static DEBUG: OnceLock<Option<Mutex<File>>> = OnceLock::new();

const MAX_BYTES: u64 = 5 * 1024 * 1024;
const BACKUP_NAME: &str = "debug.log.1";

/// Open the log once, at startup. Later calls do nothing.
pub fn init(flag: bool, home: &Path) {
    DEBUG.get_or_init(|| open_log(flag || env_on(), home));
}

/// True when `ARQUS_DEBUG` asks for debug output.
pub fn env_on() -> bool {
    match std::env::var("ARQUS_DEBUG") {
        Ok(text) => text == "1" || text.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}

/// Append one object. Sibling keys of `value` are merged next to `kind`.
pub fn log(kind: &str, value: Value) {
    let Some(Some(lock)) = DEBUG.get() else {
        return;
    };
    let key = std::env::var("ARQUS_API_KEY").unwrap_or_default();
    append_line(lock, kind, value, &key);
}

/// Open the log file when debug is on. Rotates a file over 5 MB.
/// Split out from `init` so tests can call it more than once.
fn open_log(on: bool, home: &Path) -> Option<Mutex<File>> {
    if !on {
        return None;
    }
    fs::create_dir_all(home).ok()?;
    let path = home.join("debug.log");
    if fs::metadata(&path)
        .map(|m| m.len() > MAX_BYTES)
        .unwrap_or(false)
    {
        let backup = home.join(BACKUP_NAME);
        let _ = fs::remove_file(&backup);
        let _ = fs::rename(&path, &backup);
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    Some(Mutex::new(file))
}

/// Build the line, redact it, then write it with one lock.
fn append_line(lock: &Mutex<File>, kind: &str, value: Value, key: &str) {
    let line = render(kind, &value, key);
    if let Ok(mut file) = lock.lock() {
        let _ = writeln!(file, "{line}");
    }
}

/// One JSON line: `ts` and `kind` first, then the fields of `value`.
fn render(kind: &str, value: &Value, key: &str) -> String {
    let mut object = serde_json::Map::new();
    object.insert("ts".to_string(), json!(now_ms()));
    object.insert("kind".to_string(), json!(kind));
    match value {
        Value::Object(fields) => {
            for (name, field) in fields {
                object.insert(name.clone(), field.clone());
            }
        }
        other => {
            object.insert("value".to_string(), other.clone());
        }
    }
    redact_with(&Value::Object(object).to_string(), key)
}

/// Replace the configured API key and any `sk-...` token.
pub fn redact(s: &str) -> String {
    let key = std::env::var("ARQUS_API_KEY").unwrap_or_default();
    redact_with(s, &key)
}

/// Testable form: `key` is the configured API key, used only when it has at
/// least 8 characters.
pub fn redact_with(s: &str, key: &str) -> String {
    let mut out = s.to_string();
    if key.chars().count() >= 8 {
        out = out.replace(key, "[REDACTED]");
    }
    redact_sk_tokens(&out)
}

/// `sk-` followed by 10 or more letters, digits, `-` or `_`.
fn redact_sk_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find("sk-") {
        out.push_str(&rest[..position]);
        let after = &rest[position + 3..];
        let count = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .count();
        if count >= 10 {
            out.push_str("[REDACTED]");
            let bytes: usize = after.chars().take(count).map(char::len_utf8).sum();
            rest = &after[bytes..];
        } else {
            out.push_str("sk-");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;
