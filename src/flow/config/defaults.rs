use std::collections::BTreeMap;

use super::types::{
    FlowLimits, FlowSettings, OrchestratorConfig, PriceConfig, RoutingConfig, SandboxConfig,
    WorkerConfig, WorkerLimits,
};

pub fn default_max_worker_attempts() -> u32 {
    2
}
pub fn default_max_redesigns() -> u32 {
    1
}
pub fn default_max_takeover_attempts() -> u32 {
    2
}
pub fn default_max_split_depth() -> u32 {
    1
}
pub fn default_max_planning_repairs() -> u32 {
    2
}
pub fn default_max_file_read_rounds() -> u32 {
    2
}
pub fn default_max_tasks() -> u32 {
    25
}
pub fn default_max_cloud_tokens() -> u64 {
    400000
}
pub fn default_task_timeout() -> u64 {
    600
}
pub fn default_verify_timeout() -> u64 {
    300
}
pub fn default_max_changed_lines_hard() -> u32 {
    400
}
pub fn default_whole_file_max_lines() -> u32 {
    150
}

pub fn default_flow_settings() -> FlowSettings {
    FlowSettings {
        orchestrator: Some(OrchestratorConfig {
            provider: "openrouter".to_string(),
            model: "deepseek/deepseek-v4.1-flash-1017".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            api_key_env: "ARQUS_API_KEY".to_string(),
            temperature: Some(0.2),
            max_output_tokens: Some(8192),
            price: Some(PriceConfig {
                input: 0.15,
                cached_input: Some(0.003),
                output: 0.60,
            }),
            extra_body: None,
        }),
        workers: Some(vec![WorkerConfig {
            name: "local-9b".to_string(),
            provider: "openai".to_string(),
            model: "qwen3.5:9b".to_string(),
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            api_key_env: "".to_string(),
            temperature: Some(0.1),
            max_prompt_tokens: 12000,
            max_output_tokens: 4096,
            extra_body: Some(serde_json::json!({})),
            strengths: vec![
                "boilerplate".to_string(),
                "tests".to_string(),
                "single_function".to_string(),
                "docs".to_string(),
            ],
            weaknesses: vec![
                "multi_file".to_string(),
                "ambiguous_requirements".to_string(),
            ],
            limits: WorkerLimits {
                max_context_files: 3,
                max_write_files: 2,
                max_est_changed_lines: 80,
            },
        }]),
        routing: Some(RoutingConfig {
            default_worker: "local-9b".to_string(),
            by_category: BTreeMap::new(),
        }),
        limits: Some(FlowLimits {
            max_worker_attempts: default_max_worker_attempts(),
            max_redesigns: default_max_redesigns(),
            max_takeover_attempts: default_max_takeover_attempts(),
            max_split_depth: default_max_split_depth(),
            max_planning_repairs: default_max_planning_repairs(),
            max_file_read_rounds: default_max_file_read_rounds(),
            max_tasks: default_max_tasks(),
            max_cloud_tokens_per_run: default_max_cloud_tokens(),
            task_timeout_secs: default_task_timeout(),
            verify_timeout_secs: default_verify_timeout(),
            max_changed_lines_hard: default_max_changed_lines_hard(),
            whole_file_max_lines: default_whole_file_max_lines(),
        }),
        sandbox: Some(SandboxConfig {
            allowed_commands: vec![
                "cargo test".to_string(),
                "cargo build".to_string(),
                "cargo clippy".to_string(),
                "pytest".to_string(),
                "python3 -m pytest".to_string(),
                "go test".to_string(),
                "go build".to_string(),
                "go vet".to_string(),
                "npm test".to_string(),
                "npx tsc".to_string(),
                "npx vitest".to_string(),
            ],
            protected_paths: vec![
                ".git/**".to_string(),
                ".arqus/flow/**".to_string(),
                ".github/**".to_string(),
            ],
            env_passthrough: vec![
                "PATH".to_string(),
                "HOME".to_string(),
                "CARGO_HOME".to_string(),
                "RUSTUP_HOME".to_string(),
                "GOPATH".to_string(),
                "GOCACHE".to_string(),
                "GOMODCACHE".to_string(),
                "npm_config_cache".to_string(),
            ],
        }),
    }
}
