//! Context builder for flow tasks.

use std::fs;
use std::path::Path;

use crate::flow::errors::FlowError;
use crate::flow::schema::types::{RunState, TaskContract};

/// Renders the complete, deterministic task context prompt for a task.
pub fn render_task_context(
    repo: &Path,
    contract: &TaskContract,
    state: &RunState,
    max_prompt_tokens: Option<u32>,
) -> Result<String, FlowError> {
    let mut out = String::new();

    // 1. Task Block
    out.push_str(&format!(
        "## Task: {} - {}\n\n",
        contract.id, contract.title
    ));
    out.push_str("### Objective\n");
    out.push_str(&contract.objective);
    out.push_str("\n\n");

    if !contract.constraints.is_empty() {
        out.push_str("### Constraints\n");
        for c in &contract.constraints {
            out.push_str(&format!("- {c}\n"));
        }
        out.push('\n');
    }

    if !contract.acceptance_criteria.is_empty() {
        out.push_str("### Acceptance Criteria\n");
        for ac in &contract.acceptance_criteria {
            out.push_str(&format!("- {ac}\n"));
        }
        out.push('\n');
    }

    if !contract.interface_notes.is_empty() {
        out.push_str("### Interface Notes\n");
        out.push_str(&contract.interface_notes);
        out.push_str("\n\n");
    }

    out.push_str("### Allowed Writes\n");
    for path in &contract.allowed_writes {
        let full = repo.join(path);
        if full.exists() {
            out.push_str(&format!("- {path}\n"));
        } else {
            out.push_str(&format!("- {path} (new file)\n"));
        }
    }
    out.push('\n');

    // 2. Upstream Summaries
    if !contract.depends_on.is_empty() {
        out.push_str("### Upstream Dependencies\n");
        for dep_id in &contract.depends_on {
            if let Some(dep_state) = state.tasks.get(dep_id) {
                out.push_str(&format!("- {}: {}\n", dep_id, dep_state.contract.title));
                if dep_state.changed_files.is_empty() {
                    out.push_str("  Changed files: (none)\n");
                } else {
                    let files_str = dep_state.changed_files.join(", ");
                    out.push_str(&format!("  Changed files: {files_str}\n"));
                }
            } else {
                out.push_str(&format!("- {dep_id}\n"));
            }
        }
        out.push('\n');
    }

    // 3. Context Files
    let mut file_token_sizes: Vec<(String, usize)> = Vec::new();

    for file_ref in &contract.context {
        let full = repo.join(&file_ref.path);
        if !full.exists() {
            return Err(FlowError::MissingContextFile {
                path: file_ref.path.clone(),
            });
        }

        let raw = fs::read_to_string(&full).map_err(|_| FlowError::MissingContextFile {
            path: file_ref.path.clone(),
        })?;

        let normalized = raw.replace("\r\n", "\n");
        let total_lines = normalized.lines().count();

        let mut section = String::new();
        match (file_ref.start_line, file_ref.end_line) {
            (Some(start), Some(end)) => {
                section.push_str(&format!(
                    "### FILE: {} (lines {}-{} of {})\n",
                    file_ref.path, start, end, total_lines
                ));
                let start_idx = (start.max(1) - 1) as usize;
                let end_idx = end as usize;
                let lines: Vec<&str> = normalized
                    .lines()
                    .skip(start_idx)
                    .take(end_idx.saturating_sub(start_idx))
                    .collect();
                for line in lines {
                    section.push_str(line);
                    section.push('\n');
                }
            }
            _ => {
                section.push_str(&format!(
                    "### FILE: {} (all {} lines)\n",
                    file_ref.path, total_lines
                ));
                section.push_str(&normalized);
                if !normalized.ends_with('\n') {
                    section.push('\n');
                }
            }
        }

        let section_tokens = section.chars().count().div_ceil(4);
        file_token_sizes.push((file_ref.path.clone(), section_tokens));

        out.push_str(&section);
        out.push('\n');
    }

    // 4. Budget Enforcement
    let estimated_tokens = out.chars().count().div_ceil(4);
    if let Some(max_tokens) = max_prompt_tokens {
        if estimated_tokens as u32 > max_tokens {
            let largest = file_token_sizes.iter().max_by_key(|(_, size)| *size);
            let largest_path = largest.map(|(p, _)| p.as_str()).unwrap_or("unknown");
            let largest_tokens = largest.map(|(_, s)| *s).unwrap_or(0);
            return Err(FlowError::BudgetExceeded(format!(
                "rendered prompt exceeds budget of {max_tokens} tokens (estimated {estimated_tokens} tokens); largest file: '{largest_path}' ({largest_tokens} tokens)"
            )));
        }
    }

    Ok(out)
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
