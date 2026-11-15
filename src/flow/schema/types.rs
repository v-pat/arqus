use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Boilerplate,
    SingleFunction,
    Tests,
    Docs,
    LocalRefactor,
    LocalBugfix,
    MultiFile,
    Other,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Assignee {
    Worker,
    Orchestrator,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    pub path: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct TaskContract {
    pub id: String,
    pub title: String,
    pub category: Category,
    pub assignee: Assignee,
    pub objective: String,
    pub context: Vec<FileRef>,
    pub allowed_writes: Vec<String>,
    pub depends_on: Vec<String>,
    pub constraints: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub interface_notes: String,
    pub verify: Vec<String>,
    pub est_changed_lines: u32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub summary: String,
    pub tasks: Vec<TaskContract>,
    pub final_verify: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlannerReply {
    Plan {
        summary: String,
        tasks: Vec<TaskContract>,
        final_verify: Vec<String>,
    },
    NeedFiles {
        files: Vec<FileRef>,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    NoOutput,
    Declined,
    ParseError,
    NoMatch,
    AmbiguousMatch,
    ScopeViolation,
    VerifyFailed,
    Timeout,
    ProviderError,
    BudgetExceeded,
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOutput => write!(f, "no_output"),
            Self::Declined => write!(f, "declined"),
            Self::ParseError => write!(f, "parse_error"),
            Self::NoMatch => write!(f, "no_match"),
            Self::AmbiguousMatch => write!(f, "ambiguous_match"),
            Self::ScopeViolation => write!(f, "scope_violation"),
            Self::VerifyFailed => write!(f, "verify_failed"),
            Self::Timeout => write!(f, "timeout"),
            Self::ProviderError => write!(f, "provider_error"),
            Self::BudgetExceeded => write!(f, "budget_exceeded"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FailureRecord {
    pub kind: FailureKind,
    pub detail: String,
    pub attempt: u32,
    pub executor: Assignee,
}

impl FailureRecord {
    pub fn new(
        kind: FailureKind,
        detail: impl Into<String>,
        attempt: u32,
        executor: Assignee,
    ) -> Self {
        Self {
            kind,
            detail: detail.into(),
            attempt,
            executor,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RedesignDecision {
    Revise {
        reason: String,
        revised: Box<TaskContract>,
    },
    Split {
        reason: String,
        split: Vec<TaskContract>,
    },
    Takeover {
        reason: String,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum Edit {
    Replace {
        path: String,
        search: String,
        replace: String,
    },
    File {
        path: String,
        content: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Ready,
    Running,
    Redesigning,
    Done,
    Failed,
    Skipped,
    Superseded,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Planning,
    Running,
    Finished,
    BudgetStopped,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct TaskState {
    pub contract: TaskContract,
    pub status: TaskStatus,
    pub executor: Assignee,
    pub worker_attempts: u32,
    pub redesigns: u32,
    pub takeover_attempts: u32,
    pub split_depth: u32,
    pub failures: Vec<FailureRecord>,
    pub commit: Option<String>,
    #[serde(default)]
    pub changed_files: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct RunState {
    pub run_id: String,
    pub objective: String,
    pub repo: String,
    pub base_commit: String,
    pub branch: String,
    pub status: RunStatus,
    pub tasks: BTreeMap<String, TaskState>,
    pub final_verify: Vec<String>,
    pub plan_summary: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    InvalidId,
    TitleTooLong,
    ObjectiveTooLong,
    ConstraintsTooMany,
    ConstraintTooLong,
    AcceptanceCriteriaTooMany,
    InterfaceNotesTooLong,
    InvalidLineRange,
    DuplicateId,
    TooManyTasks,
    MissingDependency,
    CycleDetected,
    MissingContextPath,
    EmptyAllowedWrites,
    InvalidAllowedWritePath,
    ProtectedPath,
    UnorderedOverlappingWrites,
    WorkerExceededContextFiles,
    WorkerExceededWriteFiles,
    WorkerExceededChangedLines,
    WorkerPromptBudgetExceeded,
    NonAllowlistedVerify,
    MissingVerifyCommand,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Issue {
    pub task_id: Option<String>,
    pub code: IssueCode,
    pub message: String,
}
