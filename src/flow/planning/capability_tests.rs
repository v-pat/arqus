use std::fs;
use tempfile::tempdir;

use super::render_capability_sheet;
use crate::flow::config::defaults::default_flow_settings;

#[test]
fn test_capability_sheet_renders_limits_strengths_and_weaknesses() {
    let settings = default_flow_settings();
    let worker = &settings.workers.unwrap()[0];

    let sheet = render_capability_sheet(worker, None);
    assert!(sheet.contains("Worker Capability: local-9b"));
    assert!(sheet.contains("Prompt budget: 12000 tokens"));
    assert!(sheet.contains("Max context files: 3"));
    assert!(sheet.contains("Max write files: 2"));
    assert!(sheet.contains("Max estimated changed lines: 80"));
    assert!(sheet.contains("Strengths:"));
    assert!(sheet.contains("boilerplate"));
    assert!(sheet.contains("Weaknesses:"));
    assert!(sheet.contains("multi_file"));
}

#[test]
fn test_capability_sheet_observed_stats_only_when_n_at_least_5() {
    let settings = default_flow_settings();
    let worker = &settings.workers.unwrap()[0];

    let dir = tempdir().unwrap();
    let stats_path = dir.path().join("profile_stats.json");

    let json_data = serde_json::json!({
        "categories": {
            "boilerplate": { "n": 10, "first_attempt_pass_rate": 0.90 },
            "single_function": { "n": 5, "first_attempt_pass_rate": 0.80 },
            "local_refactor": { "n": 4, "first_attempt_pass_rate": 0.50 }
        }
    });
    fs::write(&stats_path, json_data.to_string()).unwrap();

    let sheet = render_capability_sheet(worker, Some(&stats_path));
    assert!(sheet.contains("boilerplate"), "n=10 must be included");
    assert!(sheet.contains("single_function"), "n=5 must be included");
    assert!(
        !sheet.contains("local_refactor"),
        "n=4 must NOT be included"
    );
}

#[test]
fn test_capability_sheet_golden_snapshot() {
    let settings = default_flow_settings();
    let worker = &settings.workers.unwrap()[0];

    let sheet = render_capability_sheet(worker, None);
    let expected = r#"## Worker Capability: local-9b
- Model: openai/qwen3.5:9b
- Prompt budget: 12000 tokens
- Max context files: 3
- Max write files: 2
- Max estimated changed lines: 80
- Strengths: boilerplate, tests, single_function, docs
- Weaknesses: multi_file, ambiguous_requirements
"#;
    assert_eq!(sheet, expected);
}
