use super::*;
use crate::flow::config::types::{
    FlowLimits, OrchestratorConfig, PriceConfig, RoutingConfig, SandboxConfig, WorkerLimits,
};
use crate::flow::schema::types::{Assignee, Category};
use std::collections::BTreeMap;

fn sample_worker(name: &str) -> WorkerConfig {
    WorkerConfig {
        name: name.to_string(),
        provider: "openai".to_string(),
        model: "test-model".to_string(),
        base_url: "http://localhost:11434/v1".to_string(),
        api_key_env: "ARQUS_WORKER_KEY".to_string(),
        temperature: Some(0.2),
        max_prompt_tokens: 4000,
        max_output_tokens: 1000,
        extra_body: None,
        strengths: vec![],
        weaknesses: vec![],
        limits: WorkerLimits {
            max_context_files: 5,
            max_write_files: 2,
            max_est_changed_lines: 100,
        },
    }
}

fn sample_contract(category: Category) -> TaskContract {
    TaskContract {
        id: "T01".to_string(),
        title: "Test task".to_string(),
        category,
        assignee: Assignee::Worker,
        objective: "Test objective".to_string(),
        context: vec![],
        allowed_writes: vec!["src/lib.rs".to_string()],
        depends_on: vec![],
        constraints: vec![],
        acceptance_criteria: vec![],
        interface_notes: String::new(),
        verify: vec!["cargo check".to_string()],
        est_changed_lines: 10,
    }
}

#[test]
fn test_default_router_returns_default_worker() {
    let worker = sample_worker("default-worker");
    let router = DefaultRouter::new(worker.clone());
    let contract = sample_contract(Category::Boilerplate);
    let selected = router.select(&contract);
    assert_eq!(selected.name, "default-worker");
}

#[test]
fn test_default_router_ignores_by_category() {
    let default_worker = sample_worker("worker-default");
    let router = DefaultRouter::new(default_worker.clone());

    let c_tests = sample_contract(Category::Tests);
    let c_docs = sample_contract(Category::Docs);
    let c_refactor = sample_contract(Category::LocalRefactor);

    assert_eq!(router.select(&c_tests).name, "worker-default");
    assert_eq!(router.select(&c_docs).name, "worker-default");
    assert_eq!(router.select(&c_refactor).name, "worker-default");
}

#[test]
fn test_default_router_from_config() {
    let w1 = sample_worker("worker-1");
    let w2 = sample_worker("worker-2");
    let mut by_cat = BTreeMap::new();
    by_cat.insert("tests".to_string(), "worker-2".to_string());

    let config = ResolvedFlowConfig {
        orchestrator: OrchestratorConfig {
            provider: "openrouter".to_string(),
            model: "test-orch".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            api_key_env: "ARQUS_ORCHESTRATOR_KEY".to_string(),
            temperature: Some(0.2),
            max_output_tokens: Some(4000),
            price: Some(PriceConfig {
                input: 0.1,
                cached_input: Some(0.01),
                output: 0.2,
            }),
            extra_body: None,
        },
        workers: vec![w1.clone(), w2.clone()],
        routing: RoutingConfig {
            default_worker: "worker-1".to_string(),
            by_category: by_cat,
        },
        limits: FlowLimits {
            max_worker_attempts: 2,
            max_redesigns: 1,
            max_takeover_attempts: 2,
            max_split_depth: 1,
            max_planning_repairs: 2,
            max_file_read_rounds: 2,
            max_tasks: 25,
            max_cloud_tokens_per_run: 400000,
            task_timeout_secs: 600,
            verify_timeout_secs: 300,
            max_changed_lines_hard: 400,
            whole_file_max_lines: 150,
        },
        sandbox: SandboxConfig {
            allowed_commands: vec!["cargo check".to_string()],
            protected_paths: vec![],
            env_passthrough: vec![],
        },
        orchestrator_key: "dummy".to_string(),
        worker_keys: BTreeMap::new(),
    };

    let router = DefaultRouter::from_config(&config).expect("router creation failed");
    let contract = sample_contract(Category::Tests);
    assert_eq!(router.select(&contract).name, "worker-1");
}
