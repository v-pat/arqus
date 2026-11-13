use super::*;
use crate::types::{Block, Entry, EntryKind, Message, StopReason};

fn assistant(id: &str, parent: Option<&str>, usage: Option<Usage>) -> Entry {
    Entry {
        id: id.to_string(),
        parent_id: parent.map(str::to_string),
        ts: 0,
        kind: EntryKind::Message {
            message: Message::Assistant {
                blocks: vec![Block::Text {
                    text: "hi".to_string(),
                }],
                provider: "openai".to_string(),
                model: "m".to_string(),
                usage,
                stop: StopReason::Stop,
            },
        },
    }
}

fn usage(input: u64, output: u64, cost: Option<f64>) -> Usage {
    Usage {
        input,
        output,
        cache_read: 0,
        cache_write: 0,
        cost,
    }
}

fn compaction(id: &str, parent: Option<&str>, summary_usage: Option<Usage>) -> Entry {
    Entry {
        id: id.to_string(),
        parent_id: parent.map(str::to_string),
        ts: 0,
        kind: EntryKind::Compaction {
            summary: "s".to_string(),
            first_kept_id: "x".to_string(),
            tokens_before: 10,
            read_files: vec![],
            modified_files: vec![],
            summary_usage,
        },
    }
}

#[test]
fn totals_count_every_branch() {
    let entries = vec![
        assistant("a1", None, Some(usage(100, 10, Some(0.01)))),
        // second branch off a1
        assistant("a2", Some("a1"), Some(usage(200, 20, Some(0.02)))),
        assistant("a3", Some("a1"), Some(usage(300, 30, Some(0.03)))),
    ];
    let totals = session_totals(&entries);
    assert_eq!(totals.requests, 3);
    assert_eq!(totals.input, 600);
    assert_eq!(totals.output, 60);
    let cost = totals.cost.expect("cost");
    assert!((cost - 0.06).abs() < 1e-9, "cost was {cost}");
}

#[test]
fn compaction_summary_usage_is_counted() {
    let entries = vec![
        assistant("a1", None, Some(usage(100, 10, Some(0.01)))),
        compaction("c1", Some("a1"), Some(usage(50, 5, Some(0.005)))),
    ];
    let totals = session_totals(&entries);
    assert_eq!(totals.requests, 2);
    assert_eq!(totals.input, 150);
    assert_eq!(totals.output, 15);
    let cost = totals.cost.expect("cost");
    assert!((cost - 0.015).abs() < 1e-9, "cost was {cost}");
}

#[test]
fn entries_without_usage_exist_and_cost_stays_unknown() {
    let entries = vec![
        assistant("a1", None, None),
        assistant("a2", Some("a1"), Some(usage(10, 2, None))),
        compaction("c1", Some("a2"), None),
    ];
    let totals = session_totals(&entries);
    assert_eq!(totals.requests, 2);
    assert_eq!(totals.input, 10);
    assert_eq!(totals.output, 2);
    assert_eq!(totals.cost, None);
}

#[test]
fn since_gives_one_run() {
    let all = Totals {
        requests: 5,
        input: 500,
        output: 50,
        cache_read: 10,
        cost: Some(0.5),
    };
    let before = Totals {
        requests: 2,
        input: 200,
        output: 20,
        cache_read: 0,
        cost: Some(0.2),
    };
    let run = all.since(&before);
    assert_eq!(run.requests, 3);
    assert_eq!(run.input, 300);
    assert_eq!(run.output, 30);
    assert_eq!(run.cache_read, 10);
    assert_eq!(run.cost, Some(0.3));

    let same_cost = all.since(&all);
    assert_eq!(same_cost.cost, None, "no new cost means unknown");

    let no_costs = Totals { cost: None, ..all };
    assert_eq!(no_costs.since(&before).cost, None);
}

#[test]
fn run_cost_limit_message_exact() {
    let limits = Limits::default();
    let run = Totals {
        cost: Some(0.52),
        ..Totals::default()
    };
    assert_eq!(
        check_limits(&Totals::default(), &run, &limits).as_deref(),
        Some("cost limit reached: $0.52 of $0.50 (limits.max_cost_per_run). Raise it to continue.")
    );
}

#[test]
fn session_cost_limit_is_checked_too() {
    let limits = Limits::default();
    let session = Totals {
        cost: Some(5.5),
        ..Totals::default()
    };
    let message = check_limits(&session, &Totals::default(), &limits).expect("limit");
    assert!(message.contains("limits.max_cost_per_session"), "{message}");
}

#[test]
fn token_fallback_applies_when_no_cost_is_reported() {
    let limits = Limits {
        max_tokens_per_run: 100,
        ..Limits::default()
    };
    let run = Totals {
        requests: 1,
        input: 80,
        output: 20,
        cache_read: 0,
        cost: None,
    };
    let message = check_limits(&Totals::default(), &run, &limits).expect("limit");
    assert!(
        message.starts_with("token limit reached: 100 of 100"),
        "{message}"
    );
    assert_eq!(
        check_limits(&Totals::default(), &Totals::default(), &limits),
        None
    );
}

#[test]
fn summary_line_has_the_documented_shape() {
    let totals = Totals {
        requests: 3,
        input: 1234,
        output: 210,
        cache_read: 0,
        cost: Some(0.0042),
    };
    assert_eq!(
        summary_line(&totals, 12.34),
        "[arqus] requests=3 in=1234 out=210 cost=$0.0042 time=12.3s"
    );
    let unknown = Totals {
        cost: None,
        ..totals
    };
    let line = summary_line(&unknown, 1.0);
    assert!(!line.contains("cost="), "{line}");
}
