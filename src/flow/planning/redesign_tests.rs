use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::*;
use crate::ai::{Llm, StreamEvent};
use crate::flow::config::types::OrchestratorConfig;
use crate::flow::schema::types::{
    Assignee, Category, FailureKind, FailureRecord, RedesignDecision, TaskContract,
};
use crate::types::{Block, Message, StopReason, Usage};

fn mock_llm(responses: Vec<String>) -> (Llm, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    for resp in responses {
        queue.lock().unwrap().push_back(vec![
            StreamEvent::TextDelta(resp.clone()),
            StreamEvent::Done(Message::Assistant {
                blocks: vec![Block::Text { text: resp }],
                provider: "mock".to_string(),
                model: "orch-model".to_string(),
                usage: Some(Usage {
                    input: 150,
                    output: 80,
                    cache_read: 0,
                    cache_write: 0,
                    cost: None,
                }),
                stop: StopReason::Stop,
            }),
        ]);
    }
    (Llm::MockCapture(queue, Arc::clone(&seen)), seen)
}

fn test_orch_config() -> OrchestratorConfig {
    OrchestratorConfig {
        provider: "openrouter".to_string(),
        model: "deepseek/v3".to_string(),
        base_url: "https://openrouter.ai/api/v1".to_string(),
        api_key_env: "TEST_KEY".to_string(),
        temperature: Some(0.2),
        max_output_tokens: Some(4096),
        price: None,
        extra_body: None,
    }
}

fn test_contract() -> TaskContract {
    TaskContract {
        id: "T01".to_string(),
        title: "Test task".to_string(),
        category: Category::Boilerplate,
        assignee: Assignee::Worker,
        objective: "Write something".to_string(),
        context: vec![],
        allowed_writes: vec!["file.txt".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo test".to_string()],
        est_changed_lines: 10,
    }
}

#[tokio::test]
async fn test_redesign_parses_revise_action() {
    let reply = serde_json::json!({
        "action": "revise",
        "reason": "Missing context file",
        "revised": {
            "id": "T01",
            "title": "Revised title",
            "category": "boilerplate",
            "assignee": "worker",
            "objective": "Revised objective",
            "context": [],
            "allowed_writes": ["file.txt"],
            "depends_on": [],
            "constraints": [],
            "acceptance_criteria": [],
            "interface_notes": "",
            "verify": ["cargo test"],
            "est_changed_lines": 15
        }
    })
    .to_string();

    let (llm, _seen) = mock_llm(vec![reply]);
    let o_cfg = test_orch_config();
    let contract = test_contract();
    let failures = vec![FailureRecord::new(
        FailureKind::NoMatch,
        "no match",
        1,
        Assignee::Worker,
    )];

    let dec = redesign(
        &llm,
        &o_cfg,
        &contract,
        &failures,
        "Worker capability",
        None,
        "run-1",
    )
    .await;
    match dec {
        RedesignDecision::Revise { reason, revised } => {
            assert_eq!(reason, "Missing context file");
            assert_eq!(revised.title, "Revised title");
        }
        _ => panic!("Expected RedesignDecision::Revise"),
    }
}

#[tokio::test]
async fn test_redesign_parses_split_action() {
    let reply = serde_json::json!({
        "action": "split",
        "reason": "Task too broad, split into two",
        "split": [
            {
                "id": "T01.1",
                "title": "Subtask 1",
                "category": "boilerplate",
                "assignee": "worker",
                "objective": "Objective 1",
                "context": [],
                "allowed_writes": ["file1.txt"],
                "depends_on": [],
                "constraints": [],
                "acceptance_criteria": [],
                "interface_notes": "",
                "verify": ["cargo test"],
                "est_changed_lines": 5
            },
            {
                "id": "T01.2",
                "title": "Subtask 2",
                "category": "boilerplate",
                "assignee": "worker",
                "objective": "Objective 2",
                "context": [],
                "allowed_writes": ["file2.txt"],
                "depends_on": ["T01.1"],
                "constraints": [],
                "acceptance_criteria": [],
                "interface_notes": "",
                "verify": ["cargo test"],
                "est_changed_lines": 5
            }
        ]
    })
    .to_string();

    let (llm, _seen) = mock_llm(vec![reply]);
    let o_cfg = test_orch_config();
    let contract = test_contract();
    let failures = vec![FailureRecord::new(
        FailureKind::VerifyFailed,
        "failed",
        1,
        Assignee::Worker,
    )];

    let dec = redesign(
        &llm,
        &o_cfg,
        &contract,
        &failures,
        "Worker capability",
        None,
        "run-1",
    )
    .await;
    match dec {
        RedesignDecision::Split { reason, split } => {
            assert_eq!(reason, "Task too broad, split into two");
            assert_eq!(split.len(), 2);
            assert_eq!(split[0].id, "T01.1");
            assert_eq!(split[1].id, "T01.2");
        }
        _ => panic!("Expected RedesignDecision::Split"),
    }
}

#[tokio::test]
async fn test_redesign_parses_takeover_action() {
    let reply = serde_json::json!({
        "action": "takeover",
        "reason": "Exceeds worker abilities"
    })
    .to_string();

    let (llm, _seen) = mock_llm(vec![reply]);
    let o_cfg = test_orch_config();
    let contract = test_contract();
    let failures = vec![FailureRecord::new(
        FailureKind::Declined,
        "declined",
        1,
        Assignee::Worker,
    )];

    let dec = redesign(
        &llm,
        &o_cfg,
        &contract,
        &failures,
        "Worker capability",
        None,
        "run-1",
    )
    .await;
    match dec {
        RedesignDecision::Takeover { reason } => {
            assert_eq!(reason, "Exceeds worker abilities");
        }
        _ => panic!("Expected RedesignDecision::Takeover"),
    }
}

#[tokio::test]
async fn test_redesign_invalid_output_counts_as_takeover() {
    // Model returns gibberish or bad json
    let reply = "I don't know how to redesign this task in JSON.".to_string();
    let (llm, _seen) = mock_llm(vec![reply]);
    let o_cfg = test_orch_config();
    let contract = test_contract();
    let failures = vec![FailureRecord::new(
        FailureKind::VerifyFailed,
        "failed",
        1,
        Assignee::Worker,
    )];

    let dec = redesign(
        &llm,
        &o_cfg,
        &contract,
        &failures,
        "Worker capability",
        None,
        "run-1",
    )
    .await;
    match dec {
        RedesignDecision::Takeover { reason } => {
            assert!(reason.contains("invalid redesign output") || reason.contains("takeover"));
        }
        _ => panic!("Expected invalid output to default to Takeover"),
    }
}

#[tokio::test]
async fn test_redesign_prompt_holds_at_most_two_failure_records_and_respects_truncation() {
    let (llm, seen) = mock_llm(vec![
        serde_json::json!({"action": "takeover", "reason": "ok"}).to_string(),
    ]);
    let o_cfg = test_orch_config();
    let contract = test_contract();

    let long_detail = "x".repeat(3000);
    let failures = vec![
        FailureRecord::new(FailureKind::NoOutput, "old failure 1", 1, Assignee::Worker),
        FailureRecord::new(FailureKind::ParseError, &long_detail, 2, Assignee::Worker),
        FailureRecord::new(FailureKind::VerifyFailed, &long_detail, 3, Assignee::Worker),
    ];

    let _ = redesign(
        &llm,
        &o_cfg,
        &contract,
        &failures,
        "Cap sheet",
        None,
        "run-1",
    )
    .await;
    let req = &seen.lock().unwrap()[0];

    // Must not contain the oldest failure
    assert!(!req.contains("old failure 1"));
    // Must contain attempts 2 and 3
    assert!(req.contains("Attempt 2"));
    assert!(req.contains("Attempt 3"));

    // Detail must be capped to 1500 chars, so 3000 consecutive 'x' must NOT appear
    assert!(!req.contains(&"x".repeat(1501)));
    assert!(req.contains(&"x".repeat(1500)));
}
