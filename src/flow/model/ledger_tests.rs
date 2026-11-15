use std::fs;
use tempfile::tempdir;

use super::*;
use crate::flow::config::types::PriceConfig;

fn sample_record(role: Role, input: u64, output: u64, cost: Option<f64>) -> UsageRecord {
    UsageRecord {
        ts: 1763000000000,
        run_id: "run-123".to_string(),
        task_id: Some("T01".to_string()),
        role,
        provider: "openrouter".to_string(),
        model: "model-x".to_string(),
        input,
        output,
        cache_read: 0,
        cache_write: 0,
        estimated: false,
        cost_usd: cost,
        latency_ms: 250,
        ok: true,
    }
}

#[test]
fn records_valid_json_line_per_call() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("usage.jsonl");

    let mut ledger = Ledger::new(Some(file_path.clone()));
    let rec = sample_record(Role::Planner, 500, 100, Some(0.0012));
    ledger.record(rec.clone()).expect("record succeeds");

    let content = fs::read_to_string(&file_path).expect("read usage.jsonl");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1);

    let parsed: UsageRecord = serde_json::from_str(lines[0]).expect("valid json line");
    assert_eq!(parsed, rec);
    assert_eq!(parsed.role, Role::Planner);
}

#[test]
fn cost_uses_provider_reported_cost_when_present() {
    let price = PriceConfig {
        input: 1.0,
        cached_input: Some(0.1),
        output: 2.0,
    };
    let cost = calculate_cost(Some(0.0055), Some(&price), 10_000, 5_000, 20_000);
    assert_eq!(cost, Some(0.0055));
}

#[test]
fn cost_computed_from_price_table_with_cached_input_priced_separately() {
    let price = PriceConfig {
        input: 1.0,
        cached_input: Some(0.1),
        output: 2.0,
    };
    // 100k in ($0.10) + 50k out ($0.10) + 200k cached ($0.02) = $0.22
    let cost = calculate_cost(None, Some(&price), 100_000, 50_000, 200_000);
    assert!(cost.is_some());
    let diff = (cost.unwrap() - 0.22).abs();
    assert!(diff < 1e-6, "calculated cost was: {:?}", cost);
}

#[test]
fn cost_is_null_when_neither_reported_nor_priced() {
    let cost = calculate_cost(None, None, 1000, 500, 200);
    assert_eq!(cost, None);
}

#[test]
fn ledger_totals_aggregates_by_role() {
    let mut ledger = Ledger::new(None);
    ledger
        .record(sample_record(Role::Planner, 100, 20, Some(0.01)))
        .unwrap();
    ledger
        .record(sample_record(Role::Planner, 200, 30, Some(0.02)))
        .unwrap();
    ledger
        .record(sample_record(Role::Redesign, 150, 25, Some(0.015)))
        .unwrap();
    ledger
        .record(sample_record(Role::Takeover, 300, 50, Some(0.03)))
        .unwrap();
    ledger
        .record(sample_record(Role::Worker, 1000, 200, None))
        .unwrap();

    let totals = ledger.totals();

    let planner = totals.by_role.get(&Role::Planner).expect("planner totals");
    assert_eq!(planner.requests, 2);
    assert_eq!(planner.input, 300);
    assert_eq!(planner.output, 50);
    assert!((planner.cost_usd.unwrap() - 0.03).abs() < 1e-6);

    let worker = totals.by_role.get(&Role::Worker).expect("worker totals");
    assert_eq!(worker.requests, 1);
    assert_eq!(worker.input, 1000);
    assert_eq!(worker.output, 200);
    assert_eq!(worker.cost_usd, None);
}

#[test]
fn ledger_totals_splits_cloud_and_local() {
    let mut ledger = Ledger::new(None);
    ledger
        .record(sample_record(Role::Planner, 100, 20, Some(0.01)))
        .unwrap();
    ledger
        .record(sample_record(Role::Redesign, 150, 30, Some(0.015)))
        .unwrap();
    ledger
        .record(sample_record(Role::Worker, 500, 100, None))
        .unwrap();

    let totals = ledger.totals();

    assert_eq!(totals.cloud.requests, 2);
    assert_eq!(totals.cloud.input, 250);
    assert_eq!(totals.cloud.output, 50);
    assert!((totals.cloud.cost_usd.unwrap() - 0.025).abs() < 1e-6);

    assert_eq!(totals.local.requests, 1);
    assert_eq!(totals.local.input, 500);
    assert_eq!(totals.local.output, 100);
    assert_eq!(totals.local.cost_usd, None);

    assert_eq!(totals.total.requests, 3);
    assert_eq!(totals.total.input, 750);
    assert_eq!(totals.total.output, 150);
}

