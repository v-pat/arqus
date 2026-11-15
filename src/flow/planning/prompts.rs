use crate::flow::schema::types::Issue;
use crate::types::Message;

pub const PROMPT_VERSION: &str = "v1";

pub fn build_planner_system_prompt(capability_sheet: &str) -> String {
    format!(
        r#"You are the lead software architect planning a codebase modification.
Your goal is to break down the user's objective into a structured, executable dependency DAG of discrete tasks.

### Output Schema
Your output must be valid JSON conforming to one of the following schemas:

1. If you need to inspect existing files before planning (up to 8 files per round):
{{
  "type": "need_files",
  "files": [
    {{ "path": "path/to/file.rs", "start_line": null, "end_line": null }}
  ]
}}

2. When you have sufficient information to produce the plan:
{{
  "type": "plan",
  "summary": "High-level strategy and architecture overview",
  "tasks": [
    {{
      "id": "T01",
      "title": "Short title under 80 chars",
      "category": "boilerplate | single_function | tests | docs | local_refactor | local_bugfix | multi_file | other",
      "assignee": "worker | orchestrator",
      "objective": "Clear, imperative objective under 600 chars",
      "context": [{{ "path": "path/to/input.rs", "start_line": null, "end_line": null }}],
      "allowed_writes": ["path/to/output.rs"],
      "depends_on": [],
      "constraints": ["constraint under 200 chars"],
      "acceptance_criteria": ["criterion under 200 chars"],
      "interface_notes": "Signatures to honour under 800 chars, NO code bodies",
      "verify": ["allowlisted verify command, e.g. cargo test"],
      "est_changed_lines": 20
    }}
  ],
  "final_verify": ["cargo test"]
}}

### Rules
- Assign `worker` only to tasks inside the worker's limits and strengths; keep everything else for the `orchestrator`.
- Prefer fewer, larger valid tasks over many tiny ones.
- Never put code in contracts (interface notes should describe signatures and interfaces to honour, but never code bodies).
- List a verify command for every task (except category `docs`).
- Every verify and final_verify command must match sandbox allowlist.
- Allowed writes must be non-empty, relative, and not match protected patterns.
- Tasks whose allowed_writes overlap must be ordered by a dependency path.

{capability_sheet}
"#
    )
}

pub fn build_repo_map_message(repo_map: &str) -> Message {
    Message::User {
        text: format!("### REPOSITORY MAP\n\n{repo_map}"),
    }
}

pub fn build_objective_message(objective: &str) -> Message {
    Message::User {
        text: format!("### OBJECTIVE\n\n{objective}"),
    }
}

pub fn format_repair_message(issues: &[Issue]) -> String {
    let mut out = String::from(
        "The generated plan has the following validation issues. Please fix all issues and output a corrected, complete Plan JSON:\n\n",
    );
    for issue in issues {
        if let Some(ref tid) = issue.task_id {
            out.push_str(&format!("- Task {tid}: {}\n", issue.message));
        } else {
            out.push_str(&format!("- {}\n", issue.message));
        }
    }
    out
}

pub fn format_json_repair_message(error_detail: &str) -> String {
    format!(
        "Your output could not be parsed as valid JSON conforming to the schema:\n\n{}\n\nPlease output valid JSON only.",
        error_detail
    )
}

pub fn format_requested_files(files: &[(String, String)], max_bytes: usize) -> String {
    let marker = "\n[... truncated at 60 KB ...]";
    let budget = max_bytes.saturating_sub(marker.len());
    let mut out = String::new();

    for (path, content) in files {
        let section = format!("### FILE: {path}\n{content}\n\n");
        if out.len() + section.len() > budget {
            let available = budget.saturating_sub(out.len());
            let slice_end = section
                .char_indices()
                .map(|(idx, _)| idx)
                .take_while(|&idx| idx <= available)
                .last()
                .unwrap_or(0);
            out.push_str(&section[..slice_end]);
            out.push_str(marker);
            return out;
        }
        out.push_str(&section);
    }

    out
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
