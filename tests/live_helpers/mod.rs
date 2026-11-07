//! Shared helpers for the ignored live tests in `tests/live_openrouter.rs`.
//!
//! The key is passed through the child environment and never printed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub const MAX_COST: f64 = 0.05;
const MAX_OUTPUT_TOKENS: &str = "300";

pub struct Live {
    model: String,
    key: String,
}

/// The live tests return early, with one "skipped" line, unless the human
/// asked for live runs and supplied both values.
pub fn live() -> Option<Live> {
    if std::env::var("PIRS_LIVE").unwrap_or_default() != "1" {
        println!("skipped: PIRS_LIVE is not 1");
        return None;
    }
    let Some(key) = std::env::var("PIRS_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        println!("skipped: PIRS_API_KEY is not set");
        return None;
    };
    let Some(model) = std::env::var("PIRS_LIVE_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        println!("skipped: PIRS_LIVE_MODEL is not set");
        return None;
    };
    Some(Live { model, key })
}

/// Run the real binary with a temporary `PIRS_HOME`. All inherited `PIRS_*`
/// variables are removed first, so the human's provider and base URL cannot
/// leak into a live run.
pub fn run_live(live: &Live, project: &Path, home: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pirs"));
    for (name, _) in std::env::vars() {
        if name.starts_with("PIRS_") {
            cmd.env_remove(name);
        }
    }
    cmd.current_dir(project)
        .env("PIRS_HOME", home)
        .env("PIRS_API_KEY", &live.key)
        .env("PIRS_PROVIDER", "openrouter")
        .env("PIRS_MODEL", &live.model)
        .env("PIRS_MAX_OUTPUT_TOKENS", MAX_OUTPUT_TOKENS)
        .args(["--debug", "--yolo"])
        .args(args)
        .output()
        .expect("failed to run the pirs binary")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Last lines of the redacted debug log, for a readable failure.
pub fn debug_tail(home: &Path) -> String {
    let text = fs::read_to_string(home.join("debug.log")).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(40)..].join("\n")
}

pub fn ensure(condition: bool, home: &Path, message: String) {
    if !condition {
        panic!("{message}\n--- debug.log tail ---\n{}", debug_tail(home));
    }
}

pub fn ensure_exit_ok(output: &Output, home: &Path, label: &str) {
    let code = output.status.code();
    ensure(
        code == Some(0),
        home,
        format!(
            "{label}: exit {code:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            stdout(output),
            stderr(output)
        ),
    );
}

/// Costs from the `[pirs] ... cost=$...` summary lines.
pub fn costs(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for line in text.lines().filter(|line| line.contains("[pirs]")) {
        for word in line.split_whitespace() {
            if let Some(value) = word.strip_prefix("cost=$") {
                if let Ok(parsed) = value.parse::<f64>() {
                    out.push(parsed);
                }
            }
        }
    }
    out
}

pub fn requests(text: &str) -> Option<u64> {
    text.lines()
        .filter(|line| line.contains("[pirs]"))
        .flat_map(|line| line.split_whitespace())
        .find_map(|word| word.strip_prefix("requests="))
        .and_then(|value| value.parse().ok())
}

/// Every model-call run must report a cost, and no run may exceed the cap.
pub fn ensure_cost(output: &Output, home: &Path, label: &str) {
    let err = stderr(output);
    let found = costs(&err);
    ensure(
        !found.is_empty(),
        home,
        format!("{label}: no '[pirs] ... cost=$' line in stderr:\n{err}"),
    );
    for cost in found {
        println!("[live] {label}: cost=${cost:.4}");
        ensure(
            cost <= MAX_COST,
            home,
            format!("{label}: run cost ${cost:.4} is above ${MAX_COST:.2}:\n{err}"),
        );
    }
}

/// Newest `.jsonl` file anywhere under `<home>/sessions`.
pub fn newest_session(home: &Path) -> PathBuf {
    let mut files = Vec::new();
    collect_jsonl(&home.join("sessions"), &mut files);
    files.sort();
    files.pop().expect("no session file under <home>/sessions")
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}
