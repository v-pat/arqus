use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

use super::validator_graph::{build_transitive_deps, matches_pattern};
use crate::flow::config::types::ResolvedFlowConfig;
use crate::flow::exec::sandbox::check_command_allowed;
use crate::flow::repo::context::render_task_context;
use crate::flow::schema::types::{Assignee, Category, Issue, IssueCode, Plan, RunState, RunStatus};
use crate::flow::schema::{validate_shape, validate_task_id};

pub fn validate_plan(
    plan: &Plan,
    base_files: &[String],
    config: &ResolvedFlowConfig,
    repo_path: Option<&Path>,
) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Rule 1: Max tasks, ID well-formedness and uniqueness
    if plan.tasks.len() > config.limits.max_tasks as usize {
        issues.push(Issue {
            task_id: None,
            code: IssueCode::TooManyTasks,
            message: format!(
                "plan task count ({}) exceeds max_tasks ({})",
                plan.tasks.len(),
                config.limits.max_tasks
            ),
        });
    }

    let mut seen_ids = HashSet::new();
    let mut task_map = HashMap::new();
    for t in &plan.tasks {
        if !validate_task_id(&t.id) {
            issues.push(Issue {
                task_id: Some(t.id.clone()),
                code: IssueCode::InvalidId,
                message: format!("task id '{}' invalid", t.id),
            });
        }
        if !seen_ids.insert(t.id.clone()) {
            issues.push(Issue {
                task_id: Some(t.id.clone()),
                code: IssueCode::DuplicateId,
                message: format!("duplicate task id '{}'", t.id),
            });
        }
        task_map.insert(t.id.clone(), t);
    }

    // Rule 2: Depends_on exists and Kahn's algorithm for acyclicity
    for t in &plan.tasks {
        for dep in &t.depends_on {
            if !task_map.contains_key(dep) {
                issues.push(Issue {
                    task_id: Some(t.id.clone()),
                    code: IssueCode::MissingDependency,
                    message: format!("task '{}' depends on nonexistent task '{}'", t.id, dep),
                });
            }
        }
    }

    let (trans_deps, cycle) = build_transitive_deps(&plan.tasks);
    if let Some(cycle_ids) = cycle {
        issues.push(Issue {
            task_id: None,
            code: IssueCode::CycleDetected,
            message: format!(
                "dependency cycle detected among tasks: {}",
                cycle_ids.join(", ")
            ),
        });
    }

    let base_set: HashSet<&str> = base_files.iter().map(|s| s.as_str()).collect();

    // Rule 3: Context path exists in base commit or in transitive upstream allowed_writes
    for t in &plan.tasks {
        let mut upstream_writes = HashSet::new();
        if let Some(deps) = trans_deps.get(&t.id) {
            for dep_id in deps {
                if let Some(dep_task) = task_map.get(dep_id) {
                    for w in &dep_task.allowed_writes {
                        upstream_writes.insert(w.as_str());
                    }
                }
            }
        }
        for file in &t.context {
            if !base_set.contains(file.path.as_str())
                && !upstream_writes.contains(file.path.as_str())
            {
                issues.push(Issue {
                    task_id: Some(t.id.clone()),
                    code: IssueCode::MissingContextPath,
                    message: format!(
                        "context path '{}' in task '{}' does not exist at base commit and is not produced by an upstream task",
                        file.path, t.id
                    ),
                });
            }
        }
    }

    // Rule 4: allowed_writes non-empty, relative, no '..', not protected
    for t in &plan.tasks {
        if t.allowed_writes.is_empty() {
            issues.push(Issue {
                task_id: Some(t.id.clone()),
                code: IssueCode::EmptyAllowedWrites,
                message: format!("task '{}' has empty allowed_writes", t.id),
            });
        }
        for path in &t.allowed_writes {
            let p = Path::new(path);
            let has_parent = p.components().any(|c| matches!(c, Component::ParentDir));
            if path.starts_with('/') || path.starts_with('\\') || has_parent || path.is_empty() {
                issues.push(Issue {
                    task_id: Some(t.id.clone()),
                    code: IssueCode::InvalidAllowedWritePath,
                    message: format!("task '{}' has invalid allowed write path: '{}'", t.id, path),
                });
            }
            for prot in &config.sandbox.protected_paths {
                if matches_pattern(prot, path) {
                    issues.push(Issue {
                        task_id: Some(t.id.clone()),
                        code: IssueCode::ProtectedPath,
                        message: format!(
                            "task '{}' allowed write '{}' matches protected pattern '{}'",
                            t.id, path, prot
                        ),
                    });
                }
            }
        }
    }

    // Rule 5: Overlapping writes ordered by dependency path
    for i in 0..plan.tasks.len() {
        for j in (i + 1)..plan.tasks.len() {
            let t1 = &plan.tasks[i];
            let t2 = &plan.tasks[j];
            let t1_writes: HashSet<&str> = t1.allowed_writes.iter().map(|s| s.as_str()).collect();
            let has_overlap = t2
                .allowed_writes
                .iter()
                .any(|w| t1_writes.contains(w.as_str()));
            if has_overlap {
                let t1_dep_on_t2 = trans_deps.get(&t1.id).is_some_and(|d| d.contains(&t2.id));
                let t2_dep_on_t1 = trans_deps.get(&t2.id).is_some_and(|d| d.contains(&t1.id));
                if !t1_dep_on_t2 && !t2_dep_on_t1 {
                    issues.push(Issue {
                        task_id: Some(t1.id.clone()),
                        code: IssueCode::UnorderedOverlappingWrites,
                        message: format!(
                            "tasks '{}' and '{}' have overlapping allowed_writes but neither depends on the other",
                            t1.id, t2.id
                        ),
                    });
                }
            }
        }
    }

    // Rule 6: Worker limits & prompt budget
    let default_worker = config
        .workers
        .iter()
        .find(|w| w.name == config.routing.default_worker)
        .or_else(|| config.workers.first());

    for t in &plan.tasks {
        if t.assignee == Assignee::Worker {
            if let Some(worker) = default_worker {
                if t.context.len() > worker.limits.max_context_files as usize {
                    issues.push(Issue {
                        task_id: Some(t.id.clone()),
                        code: IssueCode::WorkerExceededContextFiles,
                        message: format!(
                            "task '{}' context files count ({}) exceeds worker limit ({})",
                            t.id,
                            t.context.len(),
                            worker.limits.max_context_files
                        ),
                    });
                }
                if t.allowed_writes.len() > worker.limits.max_write_files as usize {
                    issues.push(Issue {
                        task_id: Some(t.id.clone()),
                        code: IssueCode::WorkerExceededWriteFiles,
                        message: format!(
                            "task '{}' allowed_writes count ({}) exceeds worker limit ({})",
                            t.id,
                            t.allowed_writes.len(),
                            worker.limits.max_write_files
                        ),
                    });
                }
                if t.est_changed_lines > worker.limits.max_est_changed_lines {
                    issues.push(Issue {
                        task_id: Some(t.id.clone()),
                        code: IssueCode::WorkerExceededChangedLines,
                        message: format!(
                            "task '{}' est_changed_lines ({}) exceeds worker limit ({})",
                            t.id, t.est_changed_lines, worker.limits.max_est_changed_lines
                        ),
                    });
                }

                if let Some(repo) = repo_path {
                    let dummy_state = RunState {
                        run_id: "check".to_string(),
                        objective: "check".to_string(),
                        repo: repo.display().to_string(),
                        base_commit: "HEAD".to_string(),
                        branch: "check".to_string(),
                        status: RunStatus::Planning,
                        tasks: HashMap::new().into_iter().collect(),
                        final_verify: vec![],
                        plan_summary: String::new(),
                    };
                    if let Err(crate::flow::errors::FlowError::BudgetExceeded(msg)) =
                        render_task_context(repo, t, &dummy_state, Some(worker.max_prompt_tokens))
                    {
                        issues.push(Issue {
                            task_id: Some(t.id.clone()),
                            code: IssueCode::WorkerPromptBudgetExceeded,
                            message: format!("task '{}' prompt budget exceeded: {msg}", t.id),
                        });
                    }
                }
            }
        }
    }

    // Rule 7: Verify commands pass sandbox allowlist; worker tasks require verify unless category docs
    let dummy_cwd = Path::new(".");
    for cmd in &plan.final_verify {
        if check_command_allowed(cmd, &config.sandbox.allowed_commands, dummy_cwd, None).is_err() {
            issues.push(Issue {
                task_id: None,
                code: IssueCode::NonAllowlistedVerify,
                message: format!("final_verify command '{cmd}' is not allowed"),
            });
        }
    }

    for t in &plan.tasks {
        if t.assignee == Assignee::Worker && t.category != Category::Docs && t.verify.is_empty() {
            issues.push(Issue {
                task_id: Some(t.id.clone()),
                code: IssueCode::MissingVerifyCommand,
                message: format!(
                    "worker task '{}' requires at least one verify command",
                    t.id
                ),
            });
        }
        for cmd in &t.verify {
            if check_command_allowed(cmd, &config.sandbox.allowed_commands, dummy_cwd, None)
                .is_err()
            {
                issues.push(Issue {
                    task_id: Some(t.id.clone()),
                    code: IssueCode::NonAllowlistedVerify,
                    message: format!("task '{}' verify command '{cmd}' is not allowed", t.id),
                });
            }
        }
    }

    // Rule 8: Field length limits from Section 5
    for t in &plan.tasks {
        issues.extend(validate_shape(t));
    }

    issues
}

#[cfg(test)]
#[path = "validator_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "validator_rules_tests.rs"]
mod rules_tests;
