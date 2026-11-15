use super::{
    build_planner_system_prompt, build_redesign_system_prompt, build_redesign_user_message,
    build_repo_map_message, build_takeover_system_prompt, build_takeover_user_message,
    build_worker_system_prompt, build_worker_user_message, format_repair_message, truncate_head,
    truncate_tail, RetryFeedback, PROMPT_VERSION,
};
use crate::flow::schema::types::{Assignee, FailureKind, FailureRecord, Issue, IssueCode};
use crate::types::Message;

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

#[test]
fn test_truncate_helpers_utf8_safe() {
    let text = "🦀 rocket 🚀 test 12345";
    // Multibyte emojis
    let head = truncate_head(text, 5);
    assert_eq!(head, "🦀 roc");

    let tail = truncate_tail(text, 5);
    assert_eq!(tail, "12345");

    let short = "abc";
    assert_eq!(truncate_head(short, 10), "abc");
    assert_eq!(truncate_tail(short, 10), "abc");
}

#[test]
fn test_build_worker_prompt_initial_and_retry() {
    let sys = build_worker_system_prompt(150);
    assert!(sys.contains("SEARCH/REPLACE"));
    assert!(sys.contains("FILE"));
    assert!(sys.contains("up to 150 lines"));
    assert!(sys.contains("CANNOT_COMPLETE: <reason>"));

    let initial = build_worker_user_message("Task context here", None);
    match initial {
        Message::User { text } => {
            assert_eq!(text, "Task context here");
            assert!(!text.contains("PREVIOUS ATTEMPT FAILED"));
        }
        _ => panic!("Expected Message::User"),
    }

    let retry_fb = RetryFeedback {
        previous_output: "def bad_fn(): pass",
        failure_kind: FailureKind::VerifyFailed,
        failure_detail: "Assertion failed: expected 1 got 2",
    };
    let retry = build_worker_user_message("Task context here", Some(&retry_fb));
    match retry {
        Message::User { text } => {
            assert!(text.contains("Task context here"));
            assert!(text.contains("### PREVIOUS ATTEMPT FAILED"));
            assert!(text.contains("Failure kind: verify_failed"));
            assert!(text.contains("Assertion failed: expected 1 got 2"));
            assert!(text.contains("def bad_fn(): pass"));
            assert!(text.contains("output complete, corrected edit blocks"));
        }
        _ => panic!("Expected Message::User"),
    }
}

#[test]
fn test_build_takeover_prompt_with_failure_records() {
    let sys = build_takeover_system_prompt(200);
    assert!(sys.contains("up to 200 lines"));

    let failures = vec![
        FailureRecord::new(FailureKind::ParseError, "detail 1", 1, Assignee::Worker),
        FailureRecord::new(FailureKind::VerifyFailed, "detail 2", 2, Assignee::Worker),
        FailureRecord::new(FailureKind::NoMatch, "detail 3", 3, Assignee::Worker),
        FailureRecord::new(FailureKind::Timeout, "detail 4", 4, Assignee::Worker),
    ];

    let msg = build_takeover_user_message("Context text", &failures, None);
    match msg {
        Message::User { text } => {
            assert!(text.contains("Context text"));
            assert!(text.contains("### PRIOR ATTEMPTS FAILURE HISTORY"));
            // At most last 3 failures
            assert!(!text.contains("Attempt 1"));
            assert!(text.contains("Attempt 2"));
            assert!(text.contains("Attempt 3"));
            assert!(text.contains("Attempt 4"));
        }
        _ => panic!("Expected Message::User"),
    }
}

#[test]
fn test_build_redesign_prompt_formatting() {
    let cap = "Worker: local-9b (limits: 150 lines)";
    let sys = build_redesign_system_prompt(cap);
    assert!(sys.contains("lead architect redesigning"));
    assert!(sys.contains("Your reply carries NO code"));
    assert!(sys.contains("revise"));
    assert!(sys.contains("split"));
    assert!(sys.contains("takeover"));
    assert!(sys.contains(cap));

    let failures = vec![
        FailureRecord::new(FailureKind::ParseError, "old failure", 1, Assignee::Worker),
        FailureRecord::new(
            FailureKind::VerifyFailed,
            "failure 2 detail",
            2,
            Assignee::Worker,
        ),
        FailureRecord::new(
            FailureKind::Timeout,
            "failure 3 detail",
            3,
            Assignee::Worker,
        ),
    ];

    let msg = build_redesign_user_message("{\"id\":\"T01\"}", &failures);
    match msg {
        Message::User { text } => {
            assert!(text.contains("### CURRENT TASK CONTRACT"));
            assert!(text.contains("{\"id\":\"T01\"}"));
            assert!(text.contains("### RECENT FAILURES"));
            // Holds at most 2 failure records
            assert!(!text.contains("old failure"));
            assert!(text.contains("Attempt 2"));
            assert!(text.contains("failure 2 detail"));
            assert!(text.contains("Attempt 3"));
            assert!(text.contains("failure 3 detail"));
        }
        _ => panic!("Expected Message::User"),
    }
}
