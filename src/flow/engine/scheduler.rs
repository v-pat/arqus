use crate::flow::schema::types::{RunState, TaskStatus};

pub struct Scheduler;

impl Scheduler {
    pub fn new() -> Self {
        Self
    }

    /// Returns the lowest task ID among tasks in `Ready` status whose dependencies
    /// are all `Done`.
    pub fn next_ready(state: &RunState) -> Option<String> {
        for (id, task) in &state.tasks {
            if task.status == TaskStatus::Ready {
                let deps_done = task.contract.depends_on.iter().all(|dep_id| {
                    state
                        .tasks
                        .get(dep_id)
                        .map(|d| d.status == TaskStatus::Done)
                        .unwrap_or(false)
                });
                if deps_done {
                    return Some(id.clone());
                }
            }
        }
        None
    }

    /// Returns true if all tasks have reached a terminal state:
    /// Done, Failed, Skipped, or Superseded.
    pub fn is_finished(state: &RunState) -> bool {
        state.tasks.values().all(|t| match t.status {
            TaskStatus::Done
            | TaskStatus::Failed
            | TaskStatus::Skipped
            | TaskStatus::Superseded => true,
            TaskStatus::Pending
            | TaskStatus::Ready
            | TaskStatus::Running
            | TaskStatus::Redesigning => false,
        })
    }

    /// Returns true if every active (non-superseded) task is Done.
    pub fn all_done(state: &RunState) -> bool {
        state
            .tasks
            .values()
            .filter(|t| t.status != TaskStatus::Superseded)
            .all(|t| t.status == TaskStatus::Done)
    }
}

pub fn next_ready(state: &RunState) -> Option<String> {
    Scheduler::next_ready(state)
}

#[cfg(test)]
#[path = "scheduler_tests.rs"]
mod tests;
