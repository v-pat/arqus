use super::tests::{make_contract, test_config};
use super::validate_plan;
use crate::flow::schema::types::{Assignee, Category, FileRef, IssueCode, Plan};

#[test]
fn test_rule_5_overlapping_writes_rejected_without_ordering() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.allowed_writes = vec!["src/shared.rs".to_string()];
    let mut t2 = make_contract("T02");
    t2.allowed_writes = vec!["src/shared.rs".to_string()];
    let plan = Plan {
        summary: "Overlapping writes".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::UnorderedOverlappingWrites));
}

#[test]
fn test_rule_5_overlapping_writes_accepted_with_ordering() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.allowed_writes = vec!["src/shared.rs".to_string()];
    let mut t2 = make_contract("T02");
    t2.allowed_writes = vec!["src/shared.rs".to_string()];
    t2.depends_on = vec!["T01".to_string()];
    let plan = Plan {
        summary: "Overlapping writes with dep".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues.is_empty(), "expected no issues, got: {:?}", issues);
}

#[test]
fn test_rule_6_worker_task_over_limits_rejected() {
    let config = test_config();
    let base_files = vec![
        "src/f1.rs".to_string(),
        "src/f2.rs".to_string(),
        "src/f3.rs".to_string(),
    ];

    let mut t1 = make_contract("T01");
    t1.context = vec![
        FileRef {
            path: "src/f1.rs".to_string(),
            start_line: None,
            end_line: None,
        },
        FileRef {
            path: "src/f2.rs".to_string(),
            start_line: None,
            end_line: None,
        },
        FileRef {
            path: "src/f3.rs".to_string(),
            start_line: None,
            end_line: None,
        },
    ];

    let mut t2 = make_contract("T02");
    t2.allowed_writes = vec!["src/out1.rs".to_string(), "src/out2.rs".to_string()];

    let mut t3 = make_contract("T03");
    t3.allowed_writes = vec!["src/out3.rs".to_string()];
    t3.est_changed_lines = 100;

    let plan = Plan {
        summary: "Worker limits".to_string(),
        tasks: vec![t1, t2, t3],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::WorkerExceededContextFiles));
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::WorkerExceededWriteFiles));
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::WorkerExceededChangedLines));
}

#[test]
fn test_rule_6_orchestrator_task_over_worker_limits_accepted() {
    let config = test_config();
    let base_files = vec![
        "src/f1.rs".to_string(),
        "src/f2.rs".to_string(),
        "src/f3.rs".to_string(),
    ];

    let mut t1 = make_contract("T01");
    t1.assignee = Assignee::Orchestrator;
    t1.context = vec![
        FileRef {
            path: "src/f1.rs".to_string(),
            start_line: None,
            end_line: None,
        },
        FileRef {
            path: "src/f2.rs".to_string(),
            start_line: None,
            end_line: None,
        },
        FileRef {
            path: "src/f3.rs".to_string(),
            start_line: None,
            end_line: None,
        },
    ];
    t1.allowed_writes = vec!["src/out1.rs".to_string(), "src/out2.rs".to_string()];
    t1.est_changed_lines = 100;

    let plan = Plan {
        summary: "Orch over worker limits".to_string(),
        tasks: vec![t1],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(
        issues.is_empty(),
        "expected orchestrator task to be accepted, got: {:?}",
        issues
    );
}

#[test]
fn test_rule_7_non_allowlisted_verify_command_rejected() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.verify = vec!["curl evil.com".to_string()];
    let plan = Plan {
        summary: "Non allowlisted".to_string(),
        tasks: vec![t1],
        final_verify: vec!["rm -rf /".to_string()],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(
        issues
            .iter()
            .filter(|i| i.code == IssueCode::NonAllowlistedVerify)
            .count()
            >= 2
    );
}

#[test]
fn test_rule_7_worker_requires_verify_command_unless_docs() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.verify = vec![];
    let mut t2 = make_contract("T02");
    t2.allowed_writes = vec!["docs/README.md".to_string()];
    t2.category = Category::Docs;
    t2.verify = vec![];

    let plan = Plan {
        summary: "Verify command rule".to_string(),
        tasks: vec![t1, t2],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert_eq!(
        issues
            .iter()
            .filter(|i| i.code == IssueCode::MissingVerifyCommand)
            .count(),
        1,
        "only non-docs task should require verify"
    );
}

#[test]
fn test_rule_8_field_length_limits() {
    let config = test_config();
    let base_files = vec!["src/main.rs".to_string()];
    let mut t1 = make_contract("T01");
    t1.title = "A".repeat(81);
    let plan = Plan {
        summary: "Length limit".to_string(),
        tasks: vec![t1],
        final_verify: vec![],
    };
    let issues = validate_plan(&plan, &base_files, &config, None);
    assert!(issues.iter().any(|i| i.code == IssueCode::TitleTooLong));
}
