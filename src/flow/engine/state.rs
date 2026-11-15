use crate::flow::errors::FlowError;
use crate::flow::schema::types::{Plan, RunState, RunStatus, TaskContract, TaskState, TaskStatus};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub fn check_transition(from: TaskStatus, to: TaskStatus) -> bool {
    matches!(
        (from, to),
        (TaskStatus::Pending, TaskStatus::Ready)
            | (TaskStatus::Pending, TaskStatus::Skipped)
            | (TaskStatus::Ready, TaskStatus::Running)
            | (TaskStatus::Running, TaskStatus::Done)
            | (TaskStatus::Running, TaskStatus::Redesigning)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Redesigning, TaskStatus::Running)
            | (TaskStatus::Redesigning, TaskStatus::Superseded)
            | (TaskStatus::Redesigning, TaskStatus::Failed)
    )
}

impl TaskState {
    pub fn transition(&mut self, to: TaskStatus) -> Result<(), FlowError> {
        if !check_transition(self.status, to) {
            return Err(FlowError::InvalidTransition {
                from: format!("{:?}", self.status),
                to: format!("{to:?}"),
                task_id: self.contract.id.clone(),
            });
        }
        self.status = to;
        Ok(())
    }
}

impl RunState {
    pub fn from_plan(
        plan: &Plan,
        run_id: String,
        objective: String,
        repo: String,
        base_commit: String,
        branch: String,
    ) -> Self {
        let mut tasks = BTreeMap::new();
        for task in &plan.tasks {
            let status = if task.depends_on.is_empty() {
                TaskStatus::Ready
            } else {
                TaskStatus::Pending
            };
            tasks.insert(
                task.id.clone(),
                TaskState {
                    contract: task.clone(),
                    status,
                    executor: task.assignee,
                    worker_attempts: 0,
                    redesigns: 0,
                    takeover_attempts: 0,
                    split_depth: 0,
                    failures: vec![],
                    commit: None,
                    changed_files: vec![],
                },
            );
        }

        Self {
            run_id,
            objective,
            repo,
            base_commit,
            branch,
            status: RunStatus::Running,
            tasks,
            final_verify: plan.final_verify.clone(),
            plan_summary: plan.summary.clone(),
        }
    }

    pub fn transition_task(&mut self, task_id: &str, to: TaskStatus) -> Result<(), FlowError> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| FlowError::Config(format!("task '{task_id}' not found in run state")))?;
        task.transition(to)?;

        if matches!(
            to,
            TaskStatus::Done | TaskStatus::Failed | TaskStatus::Skipped
        ) {
            self.update_dependents();
        }

        Ok(())
    }

    pub fn update_dependents(&mut self) {
        loop {
            let mut changes = Vec::new();

            for (id, task) in &self.tasks {
                if task.status != TaskStatus::Pending {
                    continue;
                }

                let mut any_failed_or_skipped = false;
                let mut all_done = true;

                for dep_id in &task.contract.depends_on {
                    match self.tasks.get(dep_id).map(|d| d.status) {
                        Some(TaskStatus::Failed) | Some(TaskStatus::Skipped) => {
                            any_failed_or_skipped = true;
                            break;
                        }
                        Some(TaskStatus::Done) => {}
                        _ => {
                            all_done = false;
                        }
                    }
                }

                if any_failed_or_skipped {
                    changes.push((id.clone(), TaskStatus::Skipped));
                } else if all_done {
                    changes.push((id.clone(), TaskStatus::Ready));
                }
            }

            if changes.is_empty() {
                break;
            }

            for (id, new_status) in changes {
                if let Some(task) = self.tasks.get_mut(&id) {
                    let _ = task.transition(new_status);
                }
            }
        }
    }

    pub fn replace_with_subtasks(
        &mut self,
        task_id: &str,
        mut subtasks: Vec<TaskContract>,
    ) -> Result<(), FlowError> {
        let (old_split_depth, old_depends_on) = {
            let old_task = self.tasks.get_mut(task_id).ok_or_else(|| {
                FlowError::Config(format!("task '{task_id}' not found in run state"))
            })?;
            old_task.transition(TaskStatus::Superseded)?;
            (old_task.split_depth, old_task.contract.depends_on.clone())
        };

        for (i, subtask) in subtasks.iter_mut().enumerate() {
            let expected_prefix = format!("{task_id}.");
            if !subtask.id.starts_with(&expected_prefix) {
                subtask.id = format!("{task_id}.{}", i + 1);
            }
            for dep in &old_depends_on {
                if !subtask.depends_on.contains(dep) {
                    subtask.depends_on.push(dep.clone());
                }
            }
            subtask.depends_on.sort();
            subtask.depends_on.dedup();
        }

        let subtask_ids: HashSet<String> = subtasks.iter().map(|s| s.id.clone()).collect();
        let depended_on_by_siblings: HashSet<String> = subtasks
            .iter()
            .flat_map(|s| {
                s.depends_on
                    .iter()
                    .filter(|d| subtask_ids.contains(*d))
                    .cloned()
            })
            .collect();

        let mut leaf_subtask_ids: Vec<String> = subtasks
            .iter()
            .map(|s| s.id.clone())
            .filter(|id| !depended_on_by_siblings.contains(id))
            .collect();
        leaf_subtask_ids.sort();

        for (id, other_task) in self.tasks.iter_mut() {
            if id == task_id {
                continue;
            }
            if other_task
                .contract
                .depends_on
                .contains(&task_id.to_string())
            {
                other_task.contract.depends_on.retain(|dep| dep != task_id);
                for leaf_id in &leaf_subtask_ids {
                    if !other_task.contract.depends_on.contains(leaf_id) {
                        other_task.contract.depends_on.push(leaf_id.clone());
                    }
                }
                other_task.contract.depends_on.sort();
                other_task.contract.depends_on.dedup();
            }
        }

        for subtask in subtasks {
            let new_task_state = TaskState {
                contract: subtask.clone(),
                status: TaskStatus::Pending,
                executor: subtask.assignee,
                worker_attempts: 0,
                redesigns: 0,
                takeover_attempts: 0,
                split_depth: old_split_depth + 1,
                failures: vec![],
                commit: None,
                changed_files: vec![],
            };
            self.tasks.insert(subtask.id.clone(), new_task_state);
        }

        self.update_dependents();
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<(), FlowError> {
        let json_bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| FlowError::Schema(format!("failed to serialize state: {e}")))?;
        crate::atomic_write::write_atomic(path, &json_bytes).map_err(|e| {
            FlowError::Git(format!(
                "failed to write state file '{}': {e}",
                path.display()
            ))
        })
    }

    pub fn load(path: &Path) -> Result<Self, FlowError> {
        let content = std::fs::read(path).map_err(|e| {
            FlowError::Git(format!(
                "failed to read state file '{}': {e}",
                path.display()
            ))
        })?;
        serde_json::from_slice(&content).map_err(|e| {
            FlowError::Schema(format!(
                "failed to parse state file '{}': {e}",
                path.display()
            ))
        })
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "state_split_tests.rs"]
mod split_tests;
