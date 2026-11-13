//! The risky-command matcher used by `safety`.
//!
//! A determined model or script can bypass pattern matching. This list only
//! stops accidents and common bypasses.

use std::path::Path;

use super::resolve;

/// A short reason when a bash command looks destructive. `None` means the
/// command did not match the pattern list; that does not make it safe.
pub fn is_risky(command: &str, cwd: &Path, home: Option<&Path>) -> Option<String> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let raw_first = Path::new(tokens[0])
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| tokens[0].to_string());

    // Unwrap `env [FLAGS/VARS...] <cmd>`
    let (first, effective_tokens) = if raw_first == "env" {
        let mut idx = 1;
        while idx < tokens.len() && (tokens[idx].starts_with('-') || tokens[idx].contains('=')) {
            idx += 1;
        }
        if idx < tokens.len() {
            let actual = Path::new(tokens[idx])
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| tokens[idx].to_string());
            (actual, &tokens[idx..])
        } else {
            (raw_first, &tokens[..])
        }
    } else {
        (raw_first, &tokens[..])
    };

    if tokens
        .iter()
        .any(|t| *t == "sudo" || *t == "doas" || t.ends_with("/sudo") || t.ends_with("/doas"))
    {
        return Some("sudo".to_string());
    }
    if first.starts_with("mkfs") {
        return Some("mkfs".to_string());
    }
    if first == "dd"
        && effective_tokens
            .iter()
            .any(|token| token.starts_with("if=") || token.starts_with("of=/dev/"))
    {
        return Some("dd raw device access".to_string());
    }
    if first == "rm" && rm_is_risky(effective_tokens, cwd, home) {
        return Some("rm -rf on /, ~, system or *".to_string());
    }
    if first == "chmod" && chmod_is_risky(effective_tokens) {
        return Some("chmod -R 777 /".to_string());
    }
    if (first == "curl" || first == "wget") && pipes_to_shell(effective_tokens) {
        return Some("download piped to a shell".to_string());
    }
    if first == "git"
        && effective_tokens.contains(&"push")
        && effective_tokens
            .iter()
            .any(|token| *token == "--force" || *token == "-f")
    {
        return Some("git push --force".to_string());
    }

    // Detect shell nested execution like `sh -c "rm -rf /"`
    if matches!(first.as_str(), "sh" | "bash" | "zsh" | "dash" | "ksh") {
        if let Some(pos) = effective_tokens.iter().position(|t| *t == "-c") {
            if pos + 1 < effective_tokens.len() {
                let inner = effective_tokens[pos + 1..].join(" ");
                let trimmed = inner.trim_matches(|c| c == '\'' || c == '"');
                if !trimmed.is_empty() && trimmed != command {
                    if let Some(reason) = is_risky(trimmed, cwd, home) {
                        return Some(format!("nested in shell: {reason}"));
                    }
                }
            }
        }
    }

    // Detect script language inline system executions
    if matches!(first.as_str(), "python" | "python3" | "perl" | "ruby")
        && effective_tokens.iter().any(|t| *t == "-c" || *t == "-e")
        && (command.contains("os.system")
            || command.contains("subprocess")
            || command.contains("rmtree")
            || command.contains("shutil.rmtree"))
    {
        return Some("inline script shell command".to_string());
    }

    let compact: String = command.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.contains(">/dev/sd")
        || compact.contains(">/dev/nvme")
        || compact.contains(">/dev/hd")
        || compact.contains(">/dev/vd")
    {
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
                "/" | "/."
                    | "~"
                    | "~/"
                    | "$HOME"
                    | "${HOME}"
                    | "/*"
                    | "/etc"
                    | "/etc/*"
                    | "/usr"
                    | "/usr/*"
                    | "/var"
                    | "/var/*"
                    | "/bin"
                    | "/bin/*"
                    | "/boot"
                    | "/boot/*"
                    | "/root"
                    | "/root/*"
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
