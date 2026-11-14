use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use super::defaults::*;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct PriceConfig {
    pub input: f64,
    #[serde(default)]
    pub cached_input: Option<f64>,
    pub output: f64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct OrchestratorConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub price: Option<PriceConfig>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WorkerLimits {
    pub max_context_files: u32,
    pub max_write_files: u32,
    pub max_est_changed_lines: u32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WorkerConfig {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    #[serde(default)]
    pub temperature: Option<f64>,
    pub max_prompt_tokens: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub extra_body: Option<Value>,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub weaknesses: Vec<String>,
    pub limits: WorkerLimits,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RoutingConfig {
    pub default_worker: String,
    #[serde(default)]
    pub by_category: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlowLimits {
    #[serde(default = "default_max_worker_attempts")]
    pub max_worker_attempts: u32,
    #[serde(default = "default_max_redesigns")]
    pub max_redesigns: u32,
    #[serde(default = "default_max_takeover_attempts")]
    pub max_takeover_attempts: u32,
    #[serde(default = "default_max_split_depth")]
    pub max_split_depth: u32,
    #[serde(default = "default_max_planning_repairs")]
    pub max_planning_repairs: u32,
    #[serde(default = "default_max_file_read_rounds")]
    pub max_file_read_rounds: u32,
    #[serde(default = "default_max_tasks")]
    pub max_tasks: u32,
    #[serde(default = "default_max_cloud_tokens")]
    pub max_cloud_tokens_per_run: u64,
    #[serde(default = "default_task_timeout")]
    pub task_timeout_secs: u64,
    #[serde(default = "default_verify_timeout")]
    pub verify_timeout_secs: u64,
    #[serde(default = "default_max_changed_lines_hard")]
    pub max_changed_lines_hard: u32,
    #[serde(default = "default_whole_file_max_lines")]
    pub whole_file_max_lines: u32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SandboxConfig {
    pub allowed_commands: Vec<String>,
    pub protected_paths: Vec<String>,
    pub env_passthrough: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlowSettings {
    #[serde(default)]
    pub orchestrator: Option<OrchestratorConfig>,
    #[serde(default)]
    pub workers: Option<Vec<WorkerConfig>>,
    #[serde(default)]
    pub routing: Option<RoutingConfig>,
    #[serde(default)]
    pub limits: Option<FlowLimits>,
    #[serde(default)]
    pub sandbox: Option<SandboxConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct FlowCliOverrides {
    pub orchestrator_model: Option<String>,
    pub worker: Option<String>,
    pub max_cost: Option<f64>,
    pub max_cloud_tokens: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ResolvedFlowConfig {
    pub orchestrator: OrchestratorConfig,
    pub workers: Vec<WorkerConfig>,
    pub routing: RoutingConfig,
    pub limits: FlowLimits,
    pub sandbox: SandboxConfig,
    pub orchestrator_key: String,
    pub worker_keys: BTreeMap<String, String>,
}

pub fn overlay_settings(base: &mut FlowSettings, other: FlowSettings) {
    if let Some(other_orch) = other.orchestrator {
        match base.orchestrator.as_mut() {
            Some(orch) => {
                orch.provider = other_orch.provider;
                orch.model = other_orch.model;
                orch.base_url = other_orch.base_url;
                orch.api_key_env = other_orch.api_key_env;
                if other_orch.temperature.is_some() {
                    orch.temperature = other_orch.temperature;
                }
                if other_orch.max_output_tokens.is_some() {
                    orch.max_output_tokens = other_orch.max_output_tokens;
                }
                if other_orch.price.is_some() {
                    orch.price = other_orch.price;
                }
            }
            None => base.orchestrator = Some(other_orch),
        }
    }
    if other.workers.is_some() {
        base.workers = other.workers;
    }
    if let Some(other_routing) = other.routing {
        match base.routing.as_mut() {
            Some(routing) => {
                routing.default_worker = other_routing.default_worker;
                routing.by_category.extend(other_routing.by_category);
            }
            None => base.routing = Some(other_routing),
        }
    }
    if let Some(other_limits) = other.limits {
        base.limits = Some(other_limits);
    }
    if let Some(other_sandbox) = other.sandbox {
        base.sandbox = Some(other_sandbox);
    }
}
