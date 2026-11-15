use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::flow::config::types::WorkerConfig;

#[derive(Deserialize, Debug)]
struct CategoryStat {
    n: u64,
    #[serde(alias = "pass_rate")]
    first_attempt_pass_rate: f64,
}

pub fn render_capability_sheet(worker: &WorkerConfig, profile_stats_path: Option<&Path>) -> String {
    let mut out = String::new();
    out.push_str(&format!("## Worker Capability: {}\n", worker.name));
    out.push_str(&format!("- Model: {}/{}\n", worker.provider, worker.model));
    out.push_str(&format!(
        "- Prompt budget: {} tokens\n",
        worker.max_prompt_tokens
    ));
    out.push_str(&format!(
        "- Max context files: {}\n",
        worker.limits.max_context_files
    ));
    out.push_str(&format!(
        "- Max write files: {}\n",
        worker.limits.max_write_files
    ));
    out.push_str(&format!(
        "- Max estimated changed lines: {}\n",
        worker.limits.max_est_changed_lines
    ));

    let strengths_str = if worker.strengths.is_empty() {
        "(none)".to_string()
    } else {
        worker.strengths.join(", ")
    };
    out.push_str(&format!("- Strengths: {strengths_str}\n"));

    let weaknesses_str = if worker.weaknesses.is_empty() {
        "(none)".to_string()
    } else {
        worker.weaknesses.join(", ")
    };
    out.push_str(&format!("- Weaknesses: {weaknesses_str}\n"));

    if let Some(path) = profile_stats_path {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                let stats_map: Option<BTreeMap<String, CategoryStat>> =
                    if let Some(cats) = val.get("categories") {
                        serde_json::from_value(cats.clone()).ok()
                    } else {
                        serde_json::from_value(val).ok()
                    };

                if let Some(stats) = stats_map {
                    let eligible: Vec<(&String, &CategoryStat)> =
                        stats.iter().filter(|(_, s)| s.n >= 5).collect();

                    if !eligible.is_empty() {
                        out.push_str("\n### Observed Category Pass Rates (first attempt)\n");
                        for (cat, stat) in eligible {
                            let pct = (stat.first_attempt_pass_rate * 100.0).round() as u64;
                            out.push_str(&format!("- {}: {}% (n={})\n", cat, pct, stat.n));
                        }
                    }
                }
            }
        }
    }

    out
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
