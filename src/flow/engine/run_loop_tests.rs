use super::*;

#[tokio::test]
async fn test_run_flow_fails_on_nonexistent_repo() {
    let args = RunArgs {
        objective: "test".to_string(),
        repo: "/nonexistent/path/for/sure".to_string(),
        orchestrator_model: None,
        worker: None,
        max_cost: None,
        max_cloud_tokens: None,
        dry_plan: false,
        log_prompts: false,
        debug: false,
    };
    let res = run_flow(&args).await;
    assert!(res.is_err());
}
