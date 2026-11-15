//! Atomic file replacement via a temporary file in the same directory.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generates a unique temp file path in the same parent directory as `full`.
pub fn temp_path(full: &Path) -> PathBuf {
    let dir = full.parent().unwrap_or_else(|| Path::new("."));
    let name = full
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(".{name}.arqus-tmp-{}-{n}", std::process::id()))
}

/// Atomically write `content` to `path`.
/// Creates parent directories if missing.
/// Retains the file permissions of an existing file (if any).
pub fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let permissions = std::fs::metadata(path)
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.permissions());

    let temp = temp_path(path);
    if let Err(err) = std::fs::write(&temp, content) {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    if let Some(permissions) = permissions {
        if let Err(err) = std::fs::set_permissions(&temp, permissions) {
            let _ = std::fs::remove_file(&temp);
            return Err(err);
        }
    }
    if let Err(err) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
#[path = "atomic_write_tests.rs"]
mod tests;
