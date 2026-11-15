use super::*;
use serde_json::json;

#[test]
fn test_flow_event_serialization() {
    let event = FlowEvent {
        ts: 1762400000000,
        event_type: "run_started".to_string(),
        task_id: Some("T01".to_string()),
        data: json!({ "objective": "test" }),
    };

    let serialized = serde_json::to_string(&event).unwrap();
    assert!(serialized.contains("\"ts\":1762400000000"));
    assert!(serialized.contains("\"type\":\"run_started\""));
    assert!(serialized.contains("\"task_id\":\"T01\""));
    assert!(serialized.contains("\"data\":{\"objective\":\"test\"}"));
}

#[test]
fn test_append_event_writes_valid_jsonl() {
    let tmp = std::env::temp_dir().join(format!("event_test_{}.jsonl", uuid::Uuid::new_v4()));
    let event1 = FlowEvent {
        ts: 1,
        event_type: "run_started".to_string(),
        task_id: None,
        data: json!({}),
    };
    let event2 = FlowEvent {
        ts: 2,
        event_type: "task_status".to_string(),
        task_id: Some("T01".to_string()),
        data: json!({ "status": "running" }),
    };

    append_event(&tmp, &event1).unwrap();
    append_event(&tmp, &event2).unwrap();

    let content = std::fs::read_to_string(&tmp).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    let _ = std::fs::remove_file(&tmp);
}
