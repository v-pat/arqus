use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One piece of an assistant message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        args: Value,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Stop,
    ToolUse,
    Length,
    Error,
    Aborted,
}

/// Provider-neutral message. Never put provider-specific fields here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User {
        text: String,
    },
    Assistant {
        blocks: Vec<Block>,
        provider: String,
        model: String,
        usage: Option<Usage>,
        stop: StopReason,
    },
    ToolResult {
        call_id: String,
        tool_name: String,
        text: String,
        is_error: bool,
    },
}

/// One line of the session file (after the header line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub parent_id: Option<String>,
    pub ts: u64, // milliseconds since 1970
    #[serde(flatten)]
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntryKind {
    Message {
        message: Message,
    },
    ModelChange {
        provider: String,
        model: String,
    },
    Compaction {
        summary: String,
        first_kept_id: String,
        tokens_before: u64,
        read_files: Vec<String>,
        modified_files: Vec<String>,
        /// Usage of the summary call, so summaries count in the totals.
        #[serde(default)]
        summary_usage: Option<Usage>,
    },
    BranchSummary {
        summary: String,
        from_id: String,
    },
    Custom {
        custom_type: String,
        data: Value,
    },
}

/// First line of every session file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    #[serde(rename = "type")]
    pub kind: String, // always "session"
    pub version: u32,
    pub id: String,
    pub cwd: String,
    pub ts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde_json::json;

    /// Serialize a value to JSON, read it back, and return the round trip.
    fn roundtrip<T>(value: T) -> T
    where
        T: Serialize + DeserializeOwned,
    {
        let text = serde_json::to_string(&value).expect("serialize");
        serde_json::from_str(&text).expect("deserialize")
    }

    fn user_message(text: &str) -> Message {
        Message::User {
            text: text.to_string(),
        }
    }

    #[test]
    fn message_user_roundtrip() {
        let value = user_message("list the files");
        assert_eq!(value.clone(), roundtrip(value));
    }

    #[test]
    fn assistant_with_tool_call_roundtrip() {
        let value = Message::Assistant {
            blocks: vec![
                Block::Text {
                    text: "Checking.".to_string(),
                },
                Block::ToolCall {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    args: json!({ "command": "ls" }),
                },
            ],
            provider: "openai".to_string(),
            model: "some-model".to_string(),
            usage: Some(Usage {
                input: 900,
                output: 40,
                cache_read: 0,
                cache_write: 0,
                cost: Some(0.0012),
            }),
            stop: StopReason::ToolUse,
        };
        assert_eq!(value.clone(), roundtrip(value));
    }

    #[test]
    fn tool_result_roundtrip() {
        let value = Message::ToolResult {
            call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            text: "Cargo.toml\nsrc".to_string(),
            is_error: false,
        };
        assert_eq!(value.clone(), roundtrip(value));
    }

    fn entry(parent_id: Option<&str>, kind: EntryKind) -> Entry {
        Entry {
            id: "e1000001".to_string(),
            parent_id: parent_id.map(|s| s.to_string()),
            ts: 1_762_400_001_000,
            kind,
        }
    }

    #[test]
    fn entry_message_roundtrip() {
        let value = entry(
            None,
            EntryKind::Message {
                message: user_message("list the files"),
            },
        );
        assert_eq!(value.clone(), roundtrip(value));
    }

    #[test]
    fn entry_model_change_roundtrip() {
        let value = entry(
            Some("e1000001"),
            EntryKind::ModelChange {
                provider: "openai".to_string(),
                model: "some-model".to_string(),
            },
        );
        assert_eq!(value.clone(), roundtrip(value));
    }

    #[test]
    fn entry_compaction_roundtrip() {
        let value = entry(
            Some("e1000003"),
            EntryKind::Compaction {
                summary: "## Goal\n...".to_string(),
                first_kept_id: "e1000003".to_string(),
                tokens_before: 185_000,
                read_files: vec!["src/a.rs".to_string()],
                modified_files: vec![],
                summary_usage: Some(Usage {
                    input: 100,
                    output: 20,
                    cache_read: 0,
                    cache_write: 0,
                    cost: Some(0.0001),
                }),
            },
        );
        assert_eq!(value.clone(), roundtrip(value));
    }

    #[test]
    fn entry_branch_summary_roundtrip() {
        let value = entry(
            Some("e1000003"),
            EntryKind::BranchSummary {
                summary: "tried another path".to_string(),
                from_id: "e1000001".to_string(),
            },
        );
        assert_eq!(value.clone(), roundtrip(value));
    }

    #[test]
    fn entry_custom_roundtrip() {
        let value = entry(
            Some("e1000003"),
            EntryKind::Custom {
                custom_type: "note".to_string(),
                data: json!({ "text": "hello" }),
            },
        );
        assert_eq!(value.clone(), roundtrip(value));
    }

    /// Prints a real session line so the human can compare it with AGENTS.md A7.
    #[test]
    fn print_user_entry_json() {
        let value = entry(
            None,
            EntryKind::Message {
                message: user_message("list the files"),
            },
        );
        let line = serde_json::to_string(&value).expect("serialize");
        println!("user entry JSON: {line}");
        assert!(line.contains("\"type\":\"message\""));
        assert!(line.contains("\"role\":\"user\""));
    }

    /// Stage 16 added `cost` and `summary_usage`. Old files must still load.
    #[test]
    fn old_entries_without_new_cost_fields_parse() {
        let usage: Usage =
            serde_json::from_str(r#"{"input":1,"output":2,"cache_read":3,"cache_write":4}"#)
                .expect("old usage");
        assert_eq!(usage.cost, None);

        let entry: Entry = serde_json::from_str(
            r#"{"id":"e1","parent_id":null,"ts":0,"type":"compaction","summary":"s",
                "first_kept_id":"e2","tokens_before":9,"read_files":[],"modified_files":[]}"#,
        )
        .expect("old compaction");
        match entry.kind {
            EntryKind::Compaction { summary_usage, .. } => assert!(summary_usage.is_none()),
            other => panic!("expected compaction, got {other:?}"),
        }
    }

    #[test]
    fn unknown_cost_is_not_written_to_the_file() {
        let text = serde_json::to_string(&Usage::default()).expect("serialize");
        assert!(!text.contains("cost"), "unexpected cost field in {text}");
    }
}
