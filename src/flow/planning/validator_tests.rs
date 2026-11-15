use std::collections::BTreeMap;

use super::validate_plan;
use crate::flow::config::defaults::default_flow_settings;
use crate::flow::config::types::ResolvedFlowConfig;
use crate::flow::schema::types::{Assignee, Category, FileRef, IssueCode, Plan, TaskContract};

pub(super) fn test_config() -> ResolvedFlowConfig {
    let mut defaults = default_flow_settings();
    let mut limits = defaults.limits.unwrap();
    limits.max_tasks = 5;
    defaults.sandbox.as_mut().unwrap().protected_paths = vec![
        ".git/**".to_string(),
        ".arqus/**".to_string(),
        "Cargo.lock".to_string(),
    ];
    defaults.workers.as_mut().unwrap()[0]
        .limits
        .max_context_files = 2;
    defaults.workers.as_mut().unwrap()[0].limits.max_write_files = 1;
    defaults.workers.as_mut().unwrap()[0]
        .limits
        .max_est_changed_lines = 50;
    defaults.workers.as_mut().unwrap()[0].max_prompt_tokens = 500;

    ResolvedFlowConfig {
        orchestrator: defaults.orchestrator.unwrap(),
        workers: defaults.workers.unwrap(),
        routing: defaults.routing.unwrap(),
        limits,
        sandbox: defaults.sandbox.unwrap(),
        orchestrator_key: "orch-key".to_string(),
        worker_keys: BTreeMap::new(),
    }
}

pub(super) fn make_contract(id: &str) -> TaskContract {
    TaskContract {
        id: id.to_string(),
        title: "Test Task".to_string(),
        category: Category::SingleFunction,
        assignee: Assignee::Worker,
        objective: "Do something specific".to_string(),
        context: vec![FileRef {
            path: "src/main.rs".to_string(),
            start_line: None,
            end_line: None,
        }],
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo test".to_string()],
        est_changed_lines: 20,
    }
}

#[test]
fn test_valid_plan_returns_no_issues() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];
    let plan = Plan {
        summary: "Valid plan".to_string(),
        tasks: vec![make_contract("T01")],
        final_verify: vec!["cargo test".to_string()],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
}

#[test]
fn test_rule_1_ids_unique_and_well_formed_and_max_tasks() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];

    let bad_id = make_contract("bad_id");
    let plan = Plan {
        summary: "Bad ID".to_string(),
        tasks: vec![bad_id],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues.iter().any(|i| i.code == IssueCode::InvalidId));

    let t1 = make_contract("T01");
    let mut t2 = make_contract("T01");
    t2.allowed_writes = vec!["src/other.rs".to_string()];
    let plan = Plan {
        summary: "Dup ID".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues.iter().any(|i| i.code == IssueCode::DuplicateId));

    let tasks = (1..=6)
        .map(|i| {
            let mut t = make_contract(&format!("T0{i}"));
            t.allowed_writes = vec![format!("src/file_{i}.rs")];
            t
        })
        .collect();
    let plan = Plan {
        summary: "Too many".to_string(),
        tasks,
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues.iter().any(|i| i.code == IssueCode::TooManyTasks));
}

#[test]
fn test_rule_2_missing_dependency_reported() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.depends_on = vec!["T99".to_string()];
    let plan = Plan {
        summary: "Missing dep".to_string(),
        tasks: vec![t1],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::MissingDependency));
}

#[test]
fn test_rule_2_cycle_reported_with_ids_involved() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.allowed_writes = vec!["src/t1.rs".to_string()];
    t1.depends_on = vec!["T02".to_string()];
    let mut t2 = make_contract("T02");
    t2.allowed_writes = vec!["src/t2.rs".to_string()];
    t2.depends_on = vec!["T01".to_string()];
    let plan = Plan {
        summary: "Cycle".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    let cycle = issues.iter().find(|i| i.code == IssueCode::CycleDetected);
    assert!(cycle.is_some(), "expected cycle issue");
    let msg = &cycle.unwrap().message;
    assert!(
        msg.contains("T01") && msg.contains("T02"),
        "must report IDs: {msg}"
    );
}

#[test]
fn test_rule_3_context_path_must_exist_or_be_upstream_write() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.context = vec![FileRef {
        path: "src/nonexistent.rs".to_string(),
        start_line: None,
        end_line: None,
    }];
    let plan = Plan {
        summary: "Missing context".to_string(),
        tasks: vec![t1],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::MissingContextPath));
}

#[test]
fn test_rule_3_context_path_created_by_upstream_is_accepted() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.allowed_writes = vec!["src/generated.rs".to_string()];
    let mut t2 = make_contract("T02");
    t2.context = vec![FileRef {
        path: "src/generated.rs".to_string(),
        start_line: None,
        end_line: None,
    }];
    t2.allowed_writes = vec!["src/final.rs".to_string()];
    t2.depends_on = vec!["T01".to_string()];
    let plan = Plan {
        summary: "Upstream context".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues.is_empty(), "expected valid plan, got: {:?}", issues);
}

#[test]
fn test_rule_4_empty_allowed_writes_rejected() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.allowed_writes = vec![];
    let plan = Plan {
        summary: "Empty writes".to_string(),
        tasks: vec![t1],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::EmptyAllowedWrites));
}

#[test]
fn test_rule_4_relative_path_and_no_parent_dir() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.allowed_writes = vec!["/absolute/path.rs".to_string()];
    let mut t2 = make_contract("T02");
    t2.allowed_writes = vec!["src/../secret.rs".to_string()];
    let plan = Plan {
        summary: "Bad paths".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(
        issues
            .iter()
            .filter(|i| i.code == IssueCode::InvalidAllowedWritePath)
            .count()
            >= 2
    );
}

#[test]
fn test_rule_4_protected_path_rejected() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.allowed_writes = vec![".git/config".to_string()];
    let mut t2 = make_contract("T02");
    t2.allowed_writes = vec!["Cargo.lock".to_string()];
    let plan = Plan {
        summary: "Protected paths".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(
        issues
            .iter()
            .filter(|i| i.code == IssueCode::ProtectedPath)
            .count()
            >= 2
    );
}
