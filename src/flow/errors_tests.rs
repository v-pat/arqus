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
