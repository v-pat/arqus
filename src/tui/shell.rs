//! `!command` in the prompt: run it here, never through the model.

/// The command after `!`, when the input starts with one.
pub fn local_command(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let command = trimmed.strip_prefix('!')?.trim();
    if command.is_empty() {
        None
    } else {
        Some(command.to_string())
    }
}

/// The one user message the session gets for a local command.
///
/// The bash tool already truncates long output, so the text is used as is.
pub fn message(command: &str, output: &str) -> String {
    format!(
        "I ran `{command}` locally. Output:\n\n```text\n{}\n```",
        output.trim_end()
    )
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
