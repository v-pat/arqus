use super::{
    build_planner_system_prompt, build_repo_map_message, format_repair_message, PROMPT_VERSION,
};
use crate::flow::schema::types::{Issue, IssueCode};

#[test]
fn test_prompt_version_constant() {
    assert_eq!(PROMPT_VERSION, "v1");
}

#[test]
fn test_system_prompt_golden_snapshot() {
    let dummy_capability = "## Worker Capability: local-9b\n- Model: openai/qwen3.5:9b\n";
    let system = build_planner_system_prompt(dummy_capability);
    assert!(system.contains("lead software architect"));
    assert!(system.contains("output must be valid JSON"));
    assert!(system.contains("need_files"));
    assert!(system.contains("plan"));
    assert!(system.contains(dummy_capability));
    assert!(system.contains("Assign `worker` only to tasks inside the worker's limits"));
    assert!(system.contains("Prefer fewer, larger valid tasks"));
    assert!(system.contains("Never put code in contracts"));
}

#[test]
fn test_cache_prefix_identical_across_planner_calls() {
    let cap = "Worker: local-9b";
    let repo_map = "src/main.rs\n  fn main\n";

    // Round 1
    let sys_round1 = build_planner_system_prompt(cap);
    let map_msg_round1 = build_repo_map_message(repo_map);

    // Round 2 (e.g. after need-files or repair)
    let sys_round2 = build_planner_system_prompt(cap);
    let map_msg_round2 = build_repo_map_message(repo_map);

    // Must be byte-identical
    assert_eq!(sys_round1, sys_round2);
    assert_eq!(map_msg_round1, map_msg_round2);
}

#[test]
fn test_repair_message_formatting() {
    let issues = vec![
        Issue {
            task_id: Some("T01".to_string()),
            code: IssueCode::InvalidId,
            message: "invalid ID".to_string(),
        },
        Issue {
            task_id: None,
            code: IssueCode::CycleDetected,
            message: "cycle between T01, T02".to_string(),
        },
    ];
    let msg = format_repair_message(&issues);
    assert!(msg.contains("T01: invalid ID"));
    assert!(msg.contains("cycle between T01, T02"));
}
