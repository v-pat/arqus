use serde::{Deserialize, Serialize};

use super::*;
use crate::flow::errors::FlowError;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct SamplePlan {
    pub summary: String,
    pub tasks: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct TypedConfig {
    pub count: u32,
    pub enabled: bool,
}

#[test]
fn strips_balanced_think_tags() {
    let raw = "<think>internal thoughts\nline 2</think>{\"summary\": \"done\", \"tasks\": []}";
    let cleaned = strip_think_tags(raw);
    assert_eq!(cleaned.trim(), "{\"summary\": \"done\", \"tasks\": []}");
}

#[test]
fn strips_unterminated_think_tag() {
    let raw = "{\"summary\": \"done\", \"tasks\": []}<think>stream cut off here...";
    let cleaned = strip_think_tags(raw);
    assert_eq!(cleaned.trim(), "{\"summary\": \"done\", \"tasks\": []}");

    let only_think = "<think>everything is thinking";
    assert_eq!(strip_think_tags(only_think).trim(), "");
}

#[test]
fn extracts_json_from_fenced_code_block() {
    let raw = "Sure! Here is the response:\n```json\n{\n  \"summary\": \"hello\",\n  \"tasks\": []\n}\n```\nLet me know.";
    let extracted = extract_json_text(raw).expect("should extract fenced json");
    let plan: SamplePlan = serde_json::from_str(&extracted).expect("valid plan");
    assert_eq!(plan.summary, "hello");
}

#[test]
fn extracts_json_from_surrounding_prose() {
    let raw = "Here is the output: {\"summary\": \"solo\", \"tasks\": [\"T01\"]} Have a good day.";
    let extracted = extract_json_text(raw).expect("should extract json from prose");
    let plan: SamplePlan = serde_json::from_str(&extracted).expect("valid plan");
    assert_eq!(plan.summary, "solo");
    assert_eq!(plan.tasks, vec!["T01"]);
}

#[test]
fn extracts_json_from_first_balanced_object() {
    let raw = "before {\"summary\": \"outer\", \"tasks\": [\"nested: {\"]}} after";
    let extracted = extract_json_text(raw).expect("should extract balanced object");
    let plan: SamplePlan = serde_json::from_str(&extracted).expect("valid plan");
    assert_eq!(plan.summary, "outer");
    assert_eq!(plan.tasks, vec!["nested: {"]);
}

#[test]
fn rejects_json_array_when_object_required() {
    let raw = "[{\"summary\": \"one\"}, {\"summary\": \"two\"}]";
    let err = extract_json_text(raw).expect_err("should reject array");
    match err {
        FlowError::ParseError(msg) => {
            assert!(msg.to_lowercase().contains("array"), "message: {msg}");
        }
        other => panic!("expected ParseError, got {other:?}"),
    }
}

#[test]
fn serde_error_becomes_repair_message_listing_missing_field_path() {
    let raw = "{\"summary\": \"missing tasks field\"}";
    let err = parse_json_object::<SamplePlan>(raw).expect_err("should fail missing field");
    match err {
        FlowError::ParseError(repair) => {
            assert!(
                repair.contains("tasks"),
                "repair message should list field path 'tasks': {repair}"
            );
        }
        other => panic!("expected ParseError, got {other:?}"),
    }
}

#[test]
fn serde_error_becomes_repair_message_listing_unknown_field_path() {
    let raw = "{\"summary\": \"ok\", \"tasks\": [], \"unexpected_field\": 123}";
    let err = parse_json_object::<SamplePlan>(raw).expect_err("should fail unknown field");
    match err {
        FlowError::ParseError(repair) => {
            assert!(
                repair.contains("unexpected_field"),
                "repair message should list field path 'unexpected_field': {repair}"
            );
        }
        other => panic!("expected ParseError, got {other:?}"),
    }
}

#[test]
fn serde_error_becomes_repair_message_listing_type_mismatch_path() {
    let raw = "{\"count\": \"not-a-number\", \"enabled\": true}";
    let err = parse_json_object::<TypedConfig>(raw).expect_err("should fail type mismatch");
    match err {
        FlowError::ParseError(repair) => {
            assert!(
                repair.contains("count"),
                "repair message should list field path 'count': {repair}"
            );
        }
        other => panic!("expected ParseError, got {other:?}"),
    }
}
