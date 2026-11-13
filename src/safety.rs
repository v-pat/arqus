//! Safety guardrails: approvals, path checks, risky commands, git notices.
//!
//! These are guardrails, not a sandbox. A determined model or script can
//! bypass pattern matching, shell tricks and writes made by `bash`. They stop
//! accidents, not attacks. The real boundary is the Docker container (stage
//! 25).

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// How tool calls are approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Approver {
    /// Allow every tool call (`--yolo` or `approval = "auto"`).
    Auto,
    /// Refuse `bash`, `write` and `edit` unless a path check already failed.
    /// Print mode uses this when nobody can answer a prompt.
    #[default]
    DenyAll,
    /// Ask the TUI.
    Interactive,
}

/// The answer to one approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    AllowAlways,
    Deny,
}

/// Tools the user allowed for the rest of the session with `a`.
///
/// The set is shared between turns, so `a` keeps working after a prompt ends.
#[derive(Debug, Clone, Default)]
pub struct Approvals {
    always: Arc<Mutex<HashSet<String>>>,
}

impl Approvals {
    pub fn is_always(&self, tool: &str) -> bool {
        self.always
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(tool)
    }

    pub fn remember(&self, tool: &str) {
        self.always
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(tool.to_string());
    }
}

/// What a path is used for. Reads may leave the project; writes may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOp {
    Read,
    Write,
}

/// Everything `check_path` needs. Built once when arqus starts.
#[derive(Debug, Clone, Default)]
pub struct Guard {
    pub cwd: PathBuf,
    /// Folders outside `cwd` where writes are allowed (settings `allow_paths`).
    pub allow_paths: Vec<PathBuf>,
    /// The arqus home folder: settings and history. Never read by the model.
    pub arqus_home: PathBuf,
    /// The user's home folder, for `~/.ssh` and friends.
    pub user_home: Option<PathBuf>,
}

impl Guard {
    pub fn check(&self, path: &str, op: PathOp) -> Result<PathBuf, String> {
        check_path(
            &self.cwd,
            path,
            op,
            &self.allow_paths,
            &self.arqus_home,
            self.user_home.as_deref(),
        )
    }
}

/// Resolve a path and enforce the path rules. Pure: no environment reads.
///
/// Symlinks are followed before the containment test, so a link inside the
/// project that points outside counts as outside.
pub fn check_path(
    cwd: &Path,
    path: &str,
    op: PathOp,
    allow_paths: &[PathBuf],
    arqus_home: &Path,
    user_home: Option<&Path>,
) -> Result<PathBuf, String> {
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        cwd.join(path)
    };
    let resolved = resolve(candidate);

    if op == PathOp::Read {
        for dir in sensitive_dirs(arqus_home, user_home) {
            if is_under(&dir, &resolved) {
                return Err(format!(
                    "refusing to read {}: it is under the protected folder {}",
                    resolved.display(),
                    dir.display()
                ));
            }
        }
        return Ok(resolved);
    }

    let root = resolve(cwd.to_path_buf());
    if is_under(&root, &resolved) {
        return Ok(resolved);
    }
    for allowed in allow_paths {
        if is_under(&resolve(allowed.clone()), &resolved) {
            return Ok(resolved);
        }
    }
    Err(format!(
        "refusing to write {}: it is outside the working directory {} \
         (add the folder to allow_paths in settings.json to permit it)",
        resolved.display(),
        root.display()
    ))
}

/// Folders below `cwd` that the model may write, from settings `allow_paths`.
/// `~` is the user home; a relative path is relative to the project.
pub fn expand_allow_paths(values: &[String], cwd: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for value in values {
        let text = value.trim();
        if text.is_empty() {
            continue;
        }
        let path = if text == "~" {
            match home {
                Some(home) => home.to_path_buf(),
                None => continue,
            }
        } else if let Some(rest) = text.strip_prefix("~/") {
            match home {
                Some(home) => home.join(rest),
                None => continue,
            }
        } else if Path::new(text).is_absolute() {
            PathBuf::from(text)
        } else {
            cwd.join(text)
        };
        out.push(path);
    }
    out
}

fn sensitive_dirs(arqus_home: &Path, user_home: Option<&Path>) -> Vec<PathBuf> {
    let mut out = vec![resolve(arqus_home.to_path_buf())];
    if let Some(home) = user_home {
        for name in [".ssh", ".aws", ".gnupg", ".config/gcloud", ".kube"] {
            out.push(resolve(home.join(name)));
        }
    }
    out
}

fn is_under(base: &Path, path: &Path) -> bool {
    path.starts_with(base)
}

/// Canonicalize the longest existing prefix, then append the missing parts.
///
/// `canonicalize` fails when the target does not exist yet (a new file), and it
/// resolves `..` only when the folder before it exists. Walking the components
/// keeps symlink semantics: `link/..` follows `link` first.
fn resolve(path: PathBuf) -> PathBuf {
    if let Ok(done) = std::fs::canonicalize(&path) {
        return done;
    }

    let mut base = PathBuf::new();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if tail.is_empty() {
                    if let Some(parent) = base.parent() {
                        base = parent.to_path_buf();
                    }
                } else if tail.last().map(|last| last != "..").unwrap_or(false) {
                    tail.pop();
                } else {
                    tail.push("..".into());
                }
            }
            Component::Normal(name) => {
                if tail.is_empty() {
                    let candidate = base.join(name);
                    match std::fs::canonicalize(&candidate) {
                        Ok(done) => base = done,
                        Err(_) => tail.push(name.to_os_string()),
                    }
                } else {
                    tail.push(name.to_os_string());
                }
            }
            other => {
                if tail.is_empty() {
                    base.push(other.as_os_str());
                } else {
                    tail.push(other.as_os_str().to_os_string());
                }
            }
        }
    }
    for name in tail {
        base.push(name);
    }
    base
}

#[path = "safety_risky.rs"]
mod risky;
pub use risky::is_risky;

/// One startup line about git, or `None` when git is missing or the tree is
/// clean. A missing git binary is silent on purpose.
pub fn git_notice(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .arg("status")
        .arg("--porcelain")
        .output()
        .ok()?;
    if !output.status.success() {
        return Some("not a git repository: changes cannot be undone with git".to_string());
    }
    if output.stdout.iter().any(|byte| !byte.is_ascii_whitespace()) {
        return Some("uncommitted changes present".to_string());
    }
    None
}

#[cfg(test)]
#[path = "safety_tests.rs"]
mod tests;
