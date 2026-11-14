use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowError {
    Config(String),
    Schema(String),
    MissingApiKeyEnv(String),
    UnknownWorker(String),
    Malformed(String),
    MissingContextFile {
        path: String,
    },
    InvalidTransition {
        from: String,
        to: String,
        task_id: String,
    },
    PlanningFailed(String),
    ProviderError(String),
    BudgetExceeded(String),
    Git(String),
    VerificationFailed(String),
    ScopeViolation(String),
    NoOutput,
    Declined(String),
    ParseError(String),
    NoMatch(String),
    AmbiguousMatch(String),
    Timeout(String),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "configuration error: {msg}"),
            Self::Schema(msg) => write!(f, "schema validation error: {msg}"),
            Self::MissingApiKeyEnv(var) => {
                write!(f, "missing environment variable '{var}' for role API key")
            }
            Self::UnknownWorker(name) => write!(f, "unknown default worker '{name}'"),
            Self::Malformed(msg) => write!(f, "malformed flow configuration: {msg}"),
            Self::MissingContextFile { path } => {
                write!(f, "missing required context file: {path}")
            }
            Self::InvalidTransition { from, to, task_id } => {
                write!(
                    f,
                    "invalid task status transition for task '{task_id}': from {from} to {to}"
                )
            }
            Self::PlanningFailed(msg) => write!(f, "planning failed: {msg}"),
            Self::ProviderError(msg) => write!(f, "provider error: {msg}"),
            Self::BudgetExceeded(msg) => write!(f, "budget exceeded: {msg}"),
            Self::Git(msg) => write!(f, "git error: {msg}"),
            Self::VerificationFailed(msg) => write!(f, "verification failed: {msg}"),
            Self::ScopeViolation(msg) => write!(f, "scope violation: {msg}"),
            Self::NoOutput => write!(f, "model returned no output"),
            Self::Declined(reason) => write!(f, "task declined by model: {reason}"),
            Self::ParseError(msg) => write!(f, "failed to parse model output: {msg}"),
            Self::NoMatch(msg) => write!(f, "search block matched zero times: {msg}"),
            Self::AmbiguousMatch(msg) => {
                write!(f, "search block matched multiple times: {msg}")
            }
            Self::Timeout(msg) => write!(f, "command timed out: {msg}"),
        }
    }
}

impl std::error::Error for FlowError {}

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;

