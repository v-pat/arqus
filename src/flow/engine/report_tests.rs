use super::*;
use crate::flow::model::ledger::{Ledger, Role, UsageRecord};
use crate::flow::schema::types::{
    Assignee, Category, Plan, RunState, RunStatus, TaskContract, TaskStatus,
};

fn sample_task(id: &str, title: &str, assignee: Assignee) -> TaskContract {
    TaskContract {
        id: id.to_string(),
        title: title.to_string(),
        category: Category::SingleFunction,
        assignee,
        objective: "test objective".to_string(),
        context: vec![],
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 5,
    }
}

fn sample_record(
    task_id: Option<&str>,
    role: Role,
    in_tok: u64,
    out_tok: u64,
    cost: Option<f64>,
) -> UsageRecord {
    UsageRecord {
        ts: 1000,
        run_id: "test-run".to_string(),
        task_id: task_id.map(|s| s.to_string()),
        role,
        provider: "openai".to_string(),
        model: "test-model".to_string(),
        input: in_tok,
        output: out_tok,
        cache_read: 0,
        cache_write: 0,
        estimated: false,
        cost_usd: cost,
        latency_ms: 100,
        ok: true,
    }
}

#[test]
fn test_report_lists_each_task_with_executor_attempts_redesigns_tokens_status() {
    let plan = Plan {
        summary: "test plan".to_string(),
        tasks: vec![
            sample_task("T01", "Task One", Assignee::Worker),
            sample_task("T02", "Task Two", Assignee::Orchestrator),
        ],
        final_verify: vec!["cargo test".to_string()],
    };
    let mut state = RunState::from_plan(
        &plan,
        "run123".to_string(),
        "objective".to_string(),
        "/repo".to_string(),
        "commit1".to_string(),
        "arqus/flow/run123".to_string(),
    );

    // Update T01 state
    if let Some(t1) = state.tasks.get_mut("T01") {
        t1.status = TaskStatus::Done;
        t1.worker_attempts = 2;
        t1.redesigns = 1;
    }
    // Update T02 state
    if let Some(t2) = state.tasks.get_mut("T02") {
        t2.status = TaskStatus::Failed;
        t2.takeover_attempts = 1;
    }

    let mut ledger = Ledger::new(None);
    let _ = ledger.record(sample_record(Some("T01"), Role::Worker, 1200, 300, None));
    let _ = ledger.record(sample_record(
        Some("T01"),
        Role::Redesign,
        800,
        200,
        Some(0.001),
    ));
    let _ = ledger.record(sample_record(
        Some("T02"),
        Role::Takeover,
        2000,
        400,
        Some(0.005),
    ));

    let report = generate_report(&state, &ledger);

    assert_eq!(report.run_id, "run123");
    assert_eq!(report.tasks.len(), 2);

    let t1 = &report.tasks[0];
    assert_eq!(t1.id, "T01");
    assert_eq!(t1.title, "Task One");
    assert_eq!(t1.executor, Assignee::Worker);
    assert_eq!(t1.attempts, 2);
    assert_eq!(t1.redesigns, 1);
    assert_eq!(t1.input_tokens, 2000);
    assert_eq!(t1.output_tokens, 500);
    assert_eq!(t1.status, TaskStatus::Done);

    let t2 = &report.tasks[1];
    assert_eq!(t2.id, "T02");
    assert_eq!(t2.title, "Task Two");
    assert_eq!(t2.executor, Assignee::Orchestrator);
    assert_eq!(t2.attempts, 1);
    assert_eq!(t2.redesigns, 0);
    assert_eq!(t2.input_tokens, 2000);
    assert_eq!(t2.output_tokens, 400);
    assert_eq!(t2.status, TaskStatus::Failed);
}

#[test]
fn test_report_totals_equal_ledger_totals_exactly() {
    let plan = Plan {
        summary: "test plan".to_string(),
        tasks: vec![sample_task("T01", "Task One", Assignee::Worker)],
        final_verify: vec![],
    };
    let state = RunState::from_plan(
        &plan,
        "run1".to_string(),
        "obj".to_string(),
        "/repo".to_string(),
        "c1".to_string(),
        "branch1".to_string(),
    );

    let mut ledger = Ledger::new(None);
    let _ = ledger.record(sample_record(None, Role::Planner, 1000, 200, Some(0.002)));
    let _ = ledger.record(sample_record(Some("T01"), Role::Worker, 3000, 500, None));

    let report = generate_report(&state, &ledger);
    let ledger_totals = ledger.totals();

    assert_eq!(report.totals.cloud, ledger_totals.cloud);
    assert_eq!(report.totals.local, ledger_totals.local);
    assert_eq!(report.totals.total, ledger_totals.total);
    assert_eq!(report.task_counts.total, 1);
}

#[test]
fn test_render_report_formatting() {
    let plan = Plan {
        summary: "test plan".to_string(),
        tasks: vec![sample_task("T01", "Task One", Assignee::Worker)],
        final_verify: vec![],
    };
    let mut state = RunState::from_plan(
        &plan,
        "run1".to_string(),
        "obj".to_string(),
        "/repo".to_string(),
        "c1".to_string(),
        "branch1".to_string(),
    );
    state.status = RunStatus::Finished;

    let mut ledger = Ledger::new(None);
    let _ = ledger.record(sample_record(Some("T01"), Role::Worker, 100, 50, None));

    let report = generate_report(&state, &ledger);
    let rendered = render_report(&report);

    assert!(rendered.contains("Run: run1"));
    assert!(rendered.contains("Status: finished"));
    assert!(rendered.contains("T01"));
    assert!(rendered.contains("Task One"));
    assert!(rendered.contains("Totals"));
}
