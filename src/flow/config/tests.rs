use super::*;
use std::fs;
use tempfile::tempdir;

const SAMPLE_FLOW_JSON: &str = r#"{
  "flow": {
    "orchestrator": {
      "provider": "openrouter",
      "model": "deepseek/deepseek-v4.1-flash-1017",
      "base_url": "https://openrouter.ai/api/v1",
      "api_key_env": "ARQUS_ORCH_KEY",
      "temperature": 0.2,
      "max_output_tokens": 8192,
      "price": { "input": 0.15, "cached_input": 0.003, "output": 0.60 }
    },
    "workers": [
      {
        "name": "local-9b",
        "provider": "openai",
        "model": "qwen3.5:9b",
        "base_url": "http://127.0.0.1:11434/v1",
        "api_key_env": "",
        "temperature": 0.1,
        "max_prompt_tokens": 12000,
        "max_output_tokens": 4096,
        "extra_body": {},
        "strengths": ["boilerplate", "tests", "single_function", "docs"],
        "weaknesses": ["multi_file", "ambiguous_requirements"],
        "limits": { "max_context_files": 3, "max_write_files": 2, "max_est_changed_lines": 80 }
      }
    ],
    "routing": { "default_worker": "local-9b", "by_category": {} },
    "limits": {
      "max_worker_attempts": 2,
      "max_redesigns": 1,
      "max_takeover_attempts": 2,
      "max_split_depth": 1,
      "max_planning_repairs": 2,
      "max_file_read_rounds": 2,
      "max_tasks": 25,
      "max_cloud_tokens_per_run": 400000,
      "task_timeout_secs": 600,
      "verify_timeout_secs": 300,
      "max_changed_lines_hard": 400,
      "whole_file_max_lines": 150
    },
    "sandbox": {
      "allowed_commands": ["cargo test", "cargo build"],
      "protected_paths": [".git/**", ".arqus/flow/**"],
      "env_passthrough": ["PATH", "HOME"]
    }
  }
}"#;

#[test]
fn test_valid_flow_section_loads() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    fs::write(home.path().join("settings.json"), SAMPLE_FLOW_JSON).unwrap();
    std::env::set_var("ARQUS_ORCH_KEY", "test-orch-key-12345");

    let resolved = load_flow_config(home.path(), cwd.path(), &FlowCliOverrides::default()).unwrap();
    assert_eq!(resolved.orchestrator.provider, "openrouter");
    assert_eq!(
        resolved.orchestrator.model,
        "deepseek/deepseek-v4.1-flash-1017"
    );
    assert_eq!(resolved.workers.len(), 1);
    assert_eq!(resolved.workers[0].name, "local-9b");
    assert_eq!(resolved.workers[0].model, "qwen3.5:9b");
    assert_eq!(resolved.routing.default_worker, "local-9b");
    assert_eq!(resolved.orchestrator_key, "test-orch-key-12345");
    assert_eq!(resolved.worker_keys.get("local-9b").unwrap(), "local");
}

#[test]
fn test_unknown_key_rejected() {
    let bad_json = r#"{
      "orchestrator": {
        "provider": "openrouter",
        "model": "deepseek",
        "base_url": "https://openrouter.ai/api/v1",
        "api_key_env": "K",
        "unknown_key": true
      }
    }"#;
    let res = serde_json::from_str::<FlowSettings>(bad_json);
    assert!(res.is_err());
}

#[test]
fn test_unknown_default_worker_rejected() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    let json = r#"{
      "flow": {
        "routing": { "default_worker": "non-existent-worker" }
      }
    }"#;
    fs::write(home.path().join("settings.json"), json).unwrap();

    let res = load_flow_config(home.path(), cwd.path(), &FlowCliOverrides::default());
    assert!(matches!(res, Err(FlowError::UnknownWorker(_))));
}

#[test]
fn test_workers_array_replaced_not_merged() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    fs::write(home.path().join("settings.json"), SAMPLE_FLOW_JSON).unwrap();

    let project_json = r#"{
      "flow": {
        "workers": [
          {
            "name": "project-worker",
            "provider": "openai",
            "model": "qwen:7b",
            "base_url": "http://127.0.0.1:11434/v1",
            "api_key_env": "",
            "max_prompt_tokens": 8000,
            "max_output_tokens": 2048,
            "limits": { "max_context_files": 2, "max_write_files": 1, "max_est_changed_lines": 50 }
          }
        ],
        "routing": { "default_worker": "project-worker" }
      }
    }"#;
    let proj_dir = cwd.path().join(".arqus");
    fs::create_dir_all(&proj_dir).unwrap();
    fs::write(proj_dir.join("settings.json"), project_json).unwrap();

    std::env::set_var("ARQUS_ORCH_KEY", "test-orch-key-12345");
    let resolved = load_flow_config(home.path(), cwd.path(), &FlowCliOverrides::default()).unwrap();
    assert_eq!(resolved.workers.len(), 1);
    assert_eq!(resolved.workers[0].name, "project-worker");
    assert!(!resolved.workers.iter().any(|w| w.name == "local-9b"));
}

#[test]
fn test_colon_in_model_name_survives_intact() {
    let worker_json = r#"{
        "name": "local-9b",
        "provider": "openai",
        "model": "qwen3.5:9b",
        "base_url": "http://127.0.0.1:11434/v1",
        "api_key_env": "",
        "max_prompt_tokens": 12000,
        "max_output_tokens": 4096,
        "limits": { "max_context_files": 3, "max_write_files": 2, "max_est_changed_lines": 80 }
    }"#;
    let worker: WorkerConfig = serde_json::from_str(worker_json).unwrap();
    assert_eq!(worker.provider, "openai");
    assert_eq!(worker.model, "qwen3.5:9b");
}

#[test]
fn test_api_key_env_resolution_success() {
    std::env::set_var("TEST_RESOLVE_KEY_VAR", "secret-key-123");
    let resolved = resolve_api_key("TEST_RESOLVE_KEY_VAR").unwrap();
    assert_eq!(resolved, "secret-key-123");
}

#[test]
fn test_api_key_env_missing_names_variable_never_value() {
    let res = resolve_api_key("MISSING_ENV_VAR_FOR_FLOW_ROLE");
    match res {
        Err(FlowError::MissingApiKeyEnv(var_name)) => {
            assert_eq!(var_name, "MISSING_ENV_VAR_FOR_FLOW_ROLE");
            let err_msg = FlowError::MissingApiKeyEnv(var_name).to_string();
            assert!(err_msg.contains("MISSING_ENV_VAR_FOR_FLOW_ROLE"));
        }
        _ => panic!("Expected MissingApiKeyEnv error"),
    }
}

#[test]
fn test_api_key_env_empty_sends_local() {
    let resolved = resolve_api_key("").unwrap();
    assert_eq!(resolved, "local");
}

#[test]
fn test_by_category_ignored_with_one_warning() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    let json = r#"{
      "flow": {
        "routing": {
          "default_worker": "local-9b",
          "by_category": { "boilerplate": "local-9b" }
        }
      }
    }"#;
    fs::write(home.path().join("settings.json"), json).unwrap();
    std::env::set_var("ARQUS_API_KEY", "mock-key-for-test");
    let res = load_flow_config(home.path(), cwd.path(), &FlowCliOverrides::default());
    assert!(res.is_ok());
}

#[test]
fn test_cli_overrides_replace_orchestrator_model_and_worker() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    let overrides = FlowCliOverrides {
        orchestrator_model: Some("custom/orchestrator-override".to_string()),
        worker: Some("local-9b".to_string()),
        max_cost: Some(5.0),
        max_cloud_tokens: Some(100_000),
    };

    std::env::set_var("ARQUS_API_KEY", "mock-key-for-test");
    let resolved = load_flow_config(home.path(), cwd.path(), &overrides).unwrap();
    assert_eq!(resolved.orchestrator.model, "custom/orchestrator-override");
    assert_eq!(resolved.routing.default_worker, "local-9b");
    assert_eq!(resolved.limits.max_cloud_tokens_per_run, 100_000);
}

#[test]
fn test_malformed_flow_section_fails_with_config_error() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    let bad_flow_json = r#"{
      "flow": {
        "orchestrator": "not an object"
      }
    }"#;
    fs::write(home.path().join("settings.json"), bad_flow_json).unwrap();

    let res = load_flow_config(home.path(), cwd.path(), &FlowCliOverrides::default());
    assert!(matches!(res, Err(FlowError::Malformed(_))));
}

#[test]
fn test_two_role_keys_redaction() {
    let key1 = "orch-role-key-fake-12345678";
    let key2 = "worker-role-key-fake-87654321";

    let input = format!("Calling orchestrator with {key1} and worker with {key2}");
    let redacted = crate::debug::redact_with_keys(&input, &[key1, key2]);

    assert!(!redacted.contains(key1));
    assert!(!redacted.contains(key2));
    assert!(redacted.contains("[REDACTED]"));
}

#[test]
fn test_resolve_api_key_from_auth_json() {
    let home = tempdir().unwrap();
    let auth = r#"{"test_provider":{"type":"api","key":"mock-auth-key-value-123"}}"#;
    fs::write(home.path().join("auth.json"), auth).unwrap();
    let key = resolve_api_key_with_context(
        "ARQUS_NONEXISTENT_KEY_XYZ",
        "test_provider",
        Some(home.path()),
    )
    .unwrap();
    assert_eq!(key, "mock-auth-key-value-123");
}

#[test]
fn test_resolve_api_key_from_fallback_env() {
    let home = tempdir().unwrap();
    std::env::set_var("OPENROUTER_API_KEY", "mock-env-key-value-456");
    let key =
        resolve_api_key_with_context("ARQUS_NONEXISTENT_KEY_ABC", "openrouter", Some(home.path()));
    std::env::remove_var("OPENROUTER_API_KEY");
    assert_eq!(key.unwrap(), "mock-env-key-value-456");
}
