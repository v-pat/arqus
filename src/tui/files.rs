//! Project file list for `@` mentions.
//!
//! `git ls-files` inside a repository, otherwise a directory walk that skips
//! build and dependency folders. Capped so a huge tree cannot freeze the UI.

use std::path::Path;
use std::process::Command;

pub const MAX_FILES: usize = 20_000;

/// Directories the walk never enters.
pub const SKIP_DIRS: [&str; 7] = [
    ".git",
    "target",
    "node_modules",
    ".hg",
    ".svn",
    "__pycache__",
    ".cache",
];

/// Sorted project-relative paths (relative to `root`, `/` separators).
pub fn scan(root: &Path) -> Vec<String> {
    let mut files = git_files(root);
    if files.is_empty() {
        files = walk(root);
    }
    files.truncate(MAX_FILES);
    files
}

fn git_files(root: &Path) -> Vec<String> {
    if !root.join(".git").exists() {
        return Vec::new();
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split('\0')
        .filter(|path| !path.is_empty())
        .take(MAX_FILES)
        .map(str::to_string)
        .collect()
}

fn walk(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            if is_dir {
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                stack.push(path);
            } else {
                let Ok(relative) = path.strip_prefix(root) else {
                    continue;
                };
                files.push(relative.to_string_lossy().replace('\\', "/"));
                if files.len() >= MAX_FILES {
                    return files;
                }
            }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
