//! The small risky-command matcher used by `safety`.
//!
//! A determined model or script can bypass pattern matching. This list only
//! stops accidents.

use std::path::Path;

use super::resolve;
/// A short reason when a bash command looks destructive. `None` means the
/// command did not match the small pattern list; that does not make it safe.
pub fn is_risky(command: &str, cwd: &Path, home: Option<&Path>) -> Option<String> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let first = Path::new(tokens[0])
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| tokens[0].to_string());

    if tokens.contains(&"sudo") {
        return Some("sudo".to_string());
    }
    if first.starts_with("mkfs") {
        return Some("mkfs".to_string());
    }
    if first == "dd" && tokens.iter().any(|token| token.starts_with("if=")) {
        return Some("dd if=".to_string());
    }
    if first == "rm" && rm_is_risky(&tokens, cwd, home) {
        return Some("rm -rf on /, ~ or *".to_string());
    }
    if first == "chmod" && chmod_is_risky(&tokens) {
        return Some("chmod -R 777 /".to_string());
    }
    if (first == "curl" || first == "wget") && pipes_to_shell(&tokens) {
        return Some("download piped to a shell".to_string());
    }
    if first == "git"
        && tokens.contains(&"push")
        && tokens
            .iter()
            .any(|token| *token == "--force" || *token == "-f")
    {
        return Some("git push --force".to_string());
    }
    let compact: String = command.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.contains(">/dev/sd") {
        return Some("write to a raw disk device".to_string());
    }
    None
}

fn rm_is_risky(tokens: &[&str], cwd: &Path, home: Option<&Path>) -> bool {
    let flags: Vec<&str> = tokens
        .iter()
        .skip(1)
        .take_while(|token| token.starts_with('-'))
        .copied()
        .collect();
    let recursive = flags
        .iter()
        .any(|flag| flag.contains('r') || flag.contains('R'));
    let force = flags.iter().any(|flag| flag.contains('f'));
    if !(recursive && force) {
        return false;
    }
    let at_home = home.map(|home| same_dir(cwd, home)).unwrap_or(false);
    tokens
        .iter()
        .skip(1)
        .filter(|token| !token.starts_with('-'))
        .any(|target| {
            matches!(
                *target,
                "/" | "/." | "~" | "~/" | "$HOME" | "${HOME}" | "/*"
            ) || (*target == "*" && at_home)
        })
}

fn chmod_is_risky(tokens: &[&str]) -> bool {
    let recursive = tokens
        .iter()
        .skip(1)
        .take_while(|token| token.starts_with('-'))
        .any(|flag| flag.contains('R'));
    recursive && tokens.contains(&"777") && tokens.contains(&"/")
}

fn pipes_to_shell(tokens: &[&str]) -> bool {
    let Some(pipe) = tokens.iter().position(|token| *token == "|") else {
        return false;
    };
    tokens[pipe + 1..].iter().any(|token| {
        matches!(
            Path::new(token)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .as_deref(),
            Some("sh" | "bash" | "zsh" | "dash" | "ksh")
        )
    })
}

fn same_dir(a: &Path, b: &Path) -> bool {
    resolve(a.to_path_buf()) == resolve(b.to_path_buf())
}
