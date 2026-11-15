use super::*;

#[test]
fn test_flow_error_display() {
    let err = FlowError::Config("bad setting".to_string());
    assert_eq!(err.to_string(), "configuration error: bad setting");

    let err = FlowError::Schema("invalid task".to_string());
    assert_eq!(err.to_string(), "schema validation error: invalid task");

    let err = FlowError::MissingApiKeyEnv("TEST_KEY".to_string());
    assert_eq!(
        err.to_string(),
        "missing environment variable 'TEST_KEY' for role API key"
    );

    let err = FlowError::UnknownWorker("local-test".to_string());
    assert_eq!(err.to_string(), "unknown default worker 'local-test'");

    let err = FlowError::Malformed("extra key found".to_string());
    assert_eq!(
        err.to_string(),
        "malformed flow configuration: extra key found"
    );

    let err = FlowError::NoOutput;
    assert_eq!(err.to_string(), "model returned no output");

    let err = FlowError::Declined("out of scope".to_string());
    assert_eq!(err.to_string(), "task declined by model: out of scope");
}

#[test]
fn test_flow_error_to_failure_record() {
    use crate::flow::schema::types::{Assignee, FailureKind};
    let err = FlowError::Declined("out of scope".to_string());
    let record = err.to_failure_record(1, Assignee::Worker);
    assert_eq!(record.kind, FailureKind::Declined);
    assert_eq!(record.detail, "out of scope");
    assert_eq!(record.attempt, 1);
    assert_eq!(record.executor, Assignee::Worker);
}

#[test]
fn test_flow_error_exit_codes() {
    // Bad configuration or usage exits 2
    assert_eq!(FlowError::Config("bad".into()).exit_code(), 2);
    assert_eq!(FlowError::Schema("bad".into()).exit_code(), 2);
    assert_eq!(FlowError::MissingApiKeyEnv("KEY".into()).exit_code(), 2);
    assert_eq!(FlowError::UnknownWorker("w".into()).exit_code(), 2);
    assert_eq!(FlowError::Malformed("bad".into()).exit_code(), 2);

    // Other errors (run/execution failures) exit 1
    assert_eq!(FlowError::PlanningFailed("plan".into()).exit_code(), 1);
    assert_eq!(FlowError::BudgetExceeded("limit".into()).exit_code(), 1);
    assert_eq!(FlowError::Git("dirty".into()).exit_code(), 1);
    assert_eq!(FlowError::VerificationFailed("test".into()).exit_code(), 1);
    assert_eq!(FlowError::ScopeViolation("path".into()).exit_code(), 1);
    assert_eq!(FlowError::NoOutput.exit_code(), 1);
    assert_eq!(FlowError::Declined("no".into()).exit_code(), 1);
    assert_eq!(FlowError::ParseError("parse".into()).exit_code(), 1);
    assert_eq!(FlowError::NoMatch("none".into()).exit_code(), 1);
    assert_eq!(FlowError::AmbiguousMatch("many".into()).exit_code(), 1);
    assert_eq!(FlowError::Timeout("time".into()).exit_code(), 1);
    assert_eq!(FlowError::ProviderError("500".into()).exit_code(), 1);
}
