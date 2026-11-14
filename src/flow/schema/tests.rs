use super::*;

#[test]
fn test_task_id_valid_formats() {
    assert!(validate_task_id("T01"));
    assert!(validate_task_id("T123"));
    assert!(validate_task_id("T07.1"));
    assert!(validate_task_id("T07.1.2"));
}

#[test]
fn test_task_id_invalid_formats() {
    assert!(!validate_task_id("t1"));
    assert!(!validate_task_id("T1"));
    assert!(!validate_task_id("T07."));
    assert!(!validate_task_id("X07"));
    assert!(!validate_task_id("T"));
    assert!(!validate_task_id("T01."));
    assert!(!validate_task_id("T1234"));
}

#[test]
fn test_task_contract_field_length_limits() {
    let mut contract = TaskContract {
        id: "T01".to_string(),
        title: "Valid title".to_string(),
        category: Category::SingleFunction,
        assignee: Assignee::Worker,
        objective: "Valid objective".to_string(),
        context: Vec::new(),
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: Vec::new(),
        constraints: vec!["constraint 1".to_string()],
        acceptance_criteria: vec!["criterion 1".to_string()],
        interface_notes: "fn foo()".to_string(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 10,
    };
    assert!(validate_shape(&contract).is_empty());

    // Title <= 80 chars
    contract.title = "a".repeat(81);
    let issues = validate_shape(&contract);
    assert!(issues.iter().any(|i| i.code == IssueCode::TitleTooLong));
    contract.title = "a".repeat(80);
    assert!(validate_shape(&contract).is_empty());

    // Objective <= 600 chars
    contract.objective = "a".repeat(601);
    let issues = validate_shape(&contract);
    assert!(issues.iter().any(|i| i.code == IssueCode::ObjectiveTooLong));
    contract.objective = "a".repeat(600);
    assert!(validate_shape(&contract).is_empty());

    // Constraints <= 6 items, each <= 200 chars
    contract.constraints = vec!["c".to_string(); 7];
    let issues = validate_shape(&contract);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::ConstraintsTooMany));
    contract.constraints = vec!["c".repeat(201)];
    let issues = validate_shape(&contract);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::ConstraintTooLong));
    contract.constraints = vec!["c".repeat(200); 6];
    assert!(validate_shape(&contract).is_empty());

    // Acceptance criteria <= 6 items
    contract.acceptance_criteria = vec!["ac".to_string(); 7];
    let issues = validate_shape(&contract);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::AcceptanceCriteriaTooMany));
    contract.acceptance_criteria = vec!["ac".to_string(); 6];
    assert!(validate_shape(&contract).is_empty());

    // Interface notes <= 800 chars
    contract.interface_notes = "i".repeat(801);
    let issues = validate_shape(&contract);
    assert!(issues
        .iter()
        .any(|i| i.code == IssueCode::InterfaceNotesTooLong));
    contract.interface_notes = "i".repeat(800);
    assert!(validate_shape(&contract).is_empty());
}

#[test]
fn test_file_ref_line_range_both_or_neither() {
    let both = FileRef {
        path: "src/lib.rs".to_string(),
        start_line: Some(10),
        end_line: Some(20),
    };
    assert!(validate_file_ref(&both).is_empty());

    let neither = FileRef {
        path: "src/lib.rs".to_string(),
        start_line: None,
        end_line: None,
    };
    assert!(validate_file_ref(&neither).is_empty());

    let only_start = FileRef {
        path: "src/lib.rs".to_string(),
        start_line: Some(10),
        end_line: None,
    };
    let issues = validate_file_ref(&only_start);
    assert!(issues.iter().any(|i| i.code == IssueCode::InvalidLineRange));

    let only_end = FileRef {
        path: "src/lib.rs".to_string(),
        start_line: None,
        end_line: Some(20),
    };
    let issues = validate_file_ref(&only_end);
    assert!(issues.iter().any(|i| i.code == IssueCode::InvalidLineRange));

    let start_gt_end = FileRef {
        path: "src/lib.rs".to_string(),
        start_line: Some(25),
        end_line: Some(20),
    };
    let issues = validate_file_ref(&start_gt_end);
    assert!(issues.iter().any(|i| i.code == IssueCode::InvalidLineRange));
}

#[test]
fn test_schema_serde_round_trip() {
    let contract = TaskContract {
        id: "T01".to_string(),
        title: "Test contract".to_string(),
        category: Category::Boilerplate,
        assignee: Assignee::Worker,
        objective: "Create boilerplate".to_string(),
        context: vec![FileRef {
            path: "Cargo.toml".to_string(),
            start_line: Some(1),
            end_line: Some(10),
        }],
        allowed_writes: vec!["src/main.rs".to_string()],
        depends_on: vec!["T00".to_string()],
        constraints: vec!["no unwraps".to_string()],
        acceptance_criteria: vec!["compiles".to_string()],
        interface_notes: "pub fn run()".to_string(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 15,
    };

    let serialized = serde_json::to_string(&contract).unwrap();
    let deserialized: TaskContract = serde_json::from_str(&serialized).unwrap();
    assert_eq!(contract, deserialized);
}

#[test]
fn test_schema_rejects_unknown_fields() {
    let json_with_extra = r#"{
        "id": "T01",
        "title": "Test",
        "category": "tests",
        "assignee": "worker",
        "objective": "Write tests",
        "context": [],
        "allowed_writes": ["tests/test.rs"],
        "depends_on": [],
        "constraints": [],
        "acceptance_criteria": [],
        "interface_notes": "",
        "verify": ["cargo test"],
        "est_changed_lines": 10,
        "unexpected_field": 123
    }"#;
    let res = serde_json::from_str::<TaskContract>(json_with_extra);
    assert!(res.is_err());
}

#[test]
fn test_planner_reply_tagged_union() {
    let plan_json = r#"{
        "type": "plan",
        "summary": "Full plan",
        "tasks": [],
        "final_verify": ["cargo test"]
    }"#;
    let reply: PlannerReply = serde_json::from_str(plan_json).unwrap();
    match reply {
        PlannerReply::Plan {
            summary,
            final_verify,
            ..
        } => {
            assert_eq!(summary, "Full plan");
            assert_eq!(final_verify, vec!["cargo test"]);
        }
        _ => panic!("Expected Plan variant"),
    }

    let need_files_json = r#"{
        "type": "need_files",
        "files": [{"path": "src/lib.rs", "start_line": null, "end_line": null}]
    }"#;
    let reply: PlannerReply = serde_json::from_str(need_files_json).unwrap();
    match reply {
        PlannerReply::NeedFiles { files } => {
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].path, "src/lib.rs");
        }
        _ => panic!("Expected NeedFiles variant"),
    }
}

#[test]
fn test_redesign_decision_variants() {
    let revise_json = r#"{
        "action": "revise",
        "reason": "missing context",
        "revised": {
            "id": "T01",
            "title": "Revised",
            "category": "tests",
            "assignee": "worker",
            "objective": "Do tests",
            "context": [],
            "allowed_writes": ["tests/foo.rs"],
            "depends_on": [],
            "constraints": [],
            "acceptance_criteria": [],
            "interface_notes": "",
            "verify": ["cargo test"],
            "est_changed_lines": 5
        }
    }"#;
    let dec: RedesignDecision = serde_json::from_str(revise_json).unwrap();
    match dec {
        RedesignDecision::Revise { reason, revised } => {
            assert_eq!(reason, "missing context");
            assert_eq!(revised.id, "T01");
        }
        _ => panic!("Expected Revise variant"),
    }

    let takeover_json = r#"{
        "action": "takeover",
        "reason": "too hard for worker"
    }"#;
    let dec: RedesignDecision = serde_json::from_str(takeover_json).unwrap();
    match dec {
        RedesignDecision::Takeover { reason } => {
            assert_eq!(reason, "too hard for worker");
        }
        _ => panic!("Expected Takeover variant"),
    }

    // Missing required field "reason"
    let bad_json = r#"{"action": "takeover"}"#;
    assert!(serde_json::from_str::<RedesignDecision>(bad_json).is_err());
}
