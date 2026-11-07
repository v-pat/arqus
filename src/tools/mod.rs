//! The four core tools: read, write, edit, bash.
//!
//! Nothing here may panic. Every failure becomes a `ToolOutput` with
//! `is_error: true`, because the text is sent back to the model.

pub mod bash;
pub mod edit;
pub mod read;
pub mod write;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::ai::ToolDef;

/// The result of one tool call. Plain text, never a panic.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolOutput {
    pub text: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

/// The four tool definitions, with hand-written JSON schemas.
pub fn tool_defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read".to_string(),
            description: "Read a text file. Paths are relative to the working directory. Defaults to at most 2000 lines and 50000 bytes; the result says how to continue when it is cut.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path, relative or absolute." },
                    "offset": { "type": "integer", "description": "First line to read, 1-based. Default 1." },
                    "limit": { "type": "integer", "description": "Maximum number of lines. Default 2000." }
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "write".to_string(),
            description: "Create or overwrite a text file. Missing parent folders are created.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path, relative or absolute." },
                    "content": { "type": "string", "description": "Full file content." }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDef {
            name: "edit".to_string(),
            description: "Replace one exact occurrence of old_text with new_text. old_text must occur exactly once in the file.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path, relative or absolute." },
                    "old_text": { "type": "string", "description": "Exact text to replace. Include surrounding lines to make it unique." },
                    "new_text": { "type": "string", "description": "Replacement text." }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        },
        ToolDef {
            name: "bash".to_string(),
            description: "Run a shell command with `sh -c` in the working directory. Returns combined stdout and stderr and the exit code.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to run." },
                    "timeout_secs": { "type": "integer", "description": "Timeout in seconds. Default 120." }
                },
                "required": ["command"]
            }),
        },
    ]
}

/// Run one tool by name. Unknown names and bad arguments are errors, not panics.
// TODO: optional approval mode
pub async fn execute(name: &str, args: &Value, cwd: &Path) -> ToolOutput {
    // The OpenAI parser keeps unparseable arguments as their raw string.
    if let Value::String(raw) = args {
        return ToolOutput::error(format!("tool arguments were not valid JSON: {raw}"));
    }
    match name {
        "read" => read::run(args, cwd),
        "write" => write::run(args, cwd),
        "edit" => edit::run(args, cwd),
        "bash" => bash::run(args, cwd).await,
        other => ToolOutput::error(format!(
            "unknown tool `{other}`. Available tools: read, write, edit, bash"
        )),
    }
}

/// Read a required string argument.
pub fn arg_str(args: &Value, key: &str) -> Result<String, String> {
    match args.get(key) {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(format!("argument `{key}` must be a string")),
        None => Err(format!("missing required argument `{key}`")),
    }
}

/// Read an optional integer argument, or `default` when it is absent.
pub fn arg_usize(args: &Value, key: &str, default: usize) -> Result<usize, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => match value.as_u64() {
            Some(n) => Ok(n as usize),
            None => Err(format!("argument `{key}` must be a non-negative integer")),
        },
    }
}

/// Resolve a path against `cwd` unless it is already absolute.
pub fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        cwd.join(candidate)
    }
}

/// Largest prefix of `text` of at most `max` bytes, cut on a char boundary.
///
/// Slicing a `String` at a raw byte index panics when the index lands inside
/// a multi-byte character (for example `é` or an emoji). This helper backs up
/// until the cut is valid UTF-8.
pub fn truncate_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Largest suffix of `text` of at most `max` bytes, cut on a char boundary.
pub fn truncate_from_end(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}
