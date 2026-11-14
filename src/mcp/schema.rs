//! Schema pruning, token optimization, and namespace formatting for MCP tools.

use super::types::McpTool;
use crate::ai::ToolDef;
use serde_json::{json, Map, Value};

pub const MCP_PREFIX: &str = "mcp__";
pub const MAX_DESC_CHARS: usize = 200;

/// Format a tool name with its server namespace: `mcp__{server}__{tool}`.
pub fn format_tool_name(server: &str, tool: &str) -> String {
    format!("{MCP_PREFIX}{server}__{tool}")
}

/// Parse a namespaced tool name into `(server, tool)`.
pub fn parse_tool_name(name: &str) -> Option<(&str, &str)> {
    if !name.starts_with(MCP_PREFIX) {
        return None;
    }
    let rest = &name[MCP_PREFIX.len()..];
    let (server, tool) = rest.split_once("__")?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server, tool))
}

/// Convert an MCP tool definition to an arqus `ToolDef`, with schema pruning and namespace prefix.
pub fn mcp_to_tool_def(server: &str, mcp_tool: &McpTool) -> ToolDef {
    let name = format_tool_name(server, &mcp_tool.name);
    let description = clean_description(mcp_tool.description.as_deref().unwrap_or(""));
    let parameters = prune_schema(&mcp_tool.input_schema);

    ToolDef {
        name,
        description,
        parameters,
    }
}

/// Clean and compact tool descriptions to reduce token consumption.
pub fn clean_description(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "MCP tool".to_string();
    }

    // Keep first sentence if clear, or cap at MAX_DESC_CHARS.
    let first_sentence = if let Some(dot_idx) = trimmed.find(". ") {
        &trimmed[..=dot_idx]
    } else {
        trimmed
    };

    if first_sentence.len() <= MAX_DESC_CHARS {
        first_sentence.to_string()
    } else {
        let mut cut = MAX_DESC_CHARS;
        while cut > 0 && !first_sentence.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}...", first_sentence[..cut].trim_end())
    }
}

/// Prune redundant fields from JSON Schema to optimize LLM context window.
pub fn prune_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut pruned = Map::new();
            for (k, v) in map {
                // Strip metadata that models don't need for function calling.
                if matches!(k.as_str(), "$schema" | "title" | "examples" | "$id") {
                    continue;
                }

                // If description is present inside a property, prune it slightly.
                if k == "description" {
                    if let Value::String(s) = v {
                        pruned.insert(k.clone(), Value::String(clean_description(s)));
                        continue;
                    }
                }

                // Clean empty required arrays.
                if k == "required" {
                    if let Value::Array(arr) = v {
                        if arr.is_empty() {
                            continue;
                        }
                    }
                }

                pruned.insert(k.clone(), prune_schema(v));
            }

            // Ensure "type": "object" exists for top-level tools.
            if !pruned.contains_key("type") {
                pruned.insert("type".to_string(), json!("object"));
            }

            Value::Object(pruned)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(prune_schema).collect()),
        other => other.clone(),
    }
}
