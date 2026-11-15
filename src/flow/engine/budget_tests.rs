use super::*;
use crate::flow::config::defaults::default_flow_settings;
use crate::flow::config::types::FlowLimits;
use crate::flow::errors::FlowError;
use crate::flow::model::ledger::{Ledger, Role, UsageRecord};

fn default_limits() -> FlowLimits {
    default_flow_settings().limits.expect("default limits")
}

fn make_record(role: Role, in_tok: u64, out_tok: u64, cost: Option<f64>) -> UsageRecord {
    UsageRecord {
        ts: 1000,
        run_id: "test-run".to_string(),
        task_id: None,
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
fn test_check_cloud_budget_under_limits_passes() {
    let mut ledger = Ledger::new(None);
    let _ = ledger.record(make_record(Role::Planner, 1000, 500, Some(0.01)));
    let limits = default_limits();

    let res = check_cloud_budget(&ledger, &limits, Some(1.0));
    assert!(res.is_ok());
}

#[test]
fn test_check_cloud_budget_token_limit_reached_names_key() {
    let mut ledger = Ledger::new(None);
    let _ = ledger.record(make_record(Role::Planner, 5000, 5001, None));
    let mut limits = default_limits();
    limits.max_cloud_tokens_per_run = 10000;

    let res = check_cloud_budget(&ledger, &limits, None);
    assert!(matches!(res, Err(FlowError::BudgetExceeded(_))));
    if let Err(FlowError::BudgetExceeded(msg)) = res {
        assert!(
            msg.contains("limits.max_cloud_tokens_per_run"),
            "message should name limits.max_cloud_tokens_per_run: {msg}"
        );
        assert!(msg.contains("10001 of 10000"), "message: {msg}");
    }
}

#[test]
fn test_check_cloud_budget_cost_limit_reached_names_key() {
    let mut ledger = Ledger::new(None);
    let _ = ledger.record(make_record(Role::Takeover, 100, 50, Some(2.50)));
    let limits = default_limits();

    let res = check_cloud_budget(&ledger, &limits, Some(2.00));
    assert!(matches!(res, Err(FlowError::BudgetExceeded(_))));
    if let Err(FlowError::BudgetExceeded(msg)) = res {
        assert!(
            msg.contains("limits.max_cost_per_run"),
            "message should name limits.max_cost_per_run: {msg}"
        );
        assert!(msg.contains("$2.50 of $2.00"), "message: {msg}");
    }
}

#[test]
fn test_check_cloud_budget_ignores_worker_local_tokens() {
    let mut ledger = Ledger::new(None);
    // Worker spends 500,000 local tokens
    let _ = ledger.record(make_record(Role::Worker, 300000, 200000, None));
    // Planner only spends 100 cloud tokens
    let _ = ledger.record(make_record(Role::Planner, 50, 50, None));

    let mut limits = default_limits();
    limits.max_cloud_tokens_per_run = 10000;

    let res = check_cloud_budget(&ledger, &limits, None);
    assert!(
        res.is_ok(),
        "worker local tokens should not trigger cloud token limit"
    );
}
