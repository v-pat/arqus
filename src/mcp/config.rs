//! Configuration loading for MCP servers: `~/.arqus/mcp.json` and `<cwd>/.arqus/mcp.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::types::McpServerConfig;

/// Root structure of `mcp.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct McpConfigFile {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: BTreeMap<String, McpServerConfig>,
}

/// Resolved MCP configuration mapping server name to config.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct McpConfig {
    pub servers: BTreeMap<String, McpServerConfig>,
}

impl McpConfig {
    /// Load MCP configuration using the real ARQUS_HOME and current project directory.
    pub fn load(cwd: &Path) -> Self {
        Self::load_in(&crate::home::arqus_home(), cwd)
    }

    /// Load MCP configuration with an explicit home folder (for testing).
    pub fn load_in(home: &Path, cwd: &Path) -> Self {
        let mut servers = BTreeMap::new();

        // 1. Global config: <home>/mcp.json
        let global_path = home.join("mcp.json");
        Self::read_into(&global_path, &mut servers);

        // 2. Project config: <cwd>/.arqus/mcp.json or <cwd>/mcp.json
        let project_paths = [cwd.join(".arqus").join("mcp.json"), cwd.join("mcp.json")];

        for path in project_paths {
            if path.exists() {
                Self::read_into(&path, &mut servers);
                break;
            }
        }

        // Expand environment variables in env maps.
        for config in servers.values_mut() {
            expand_env(&mut config.env);
        }

        Self { servers }
    }

    fn read_into(path: &Path, servers: &mut BTreeMap<String, McpServerConfig>) {
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        match serde_json::from_str::<McpConfigFile>(&text) {
            Ok(file) => {
                for (name, conf) in file.mcp_servers {
                    servers.insert(name, conf);
                }
            }
            Err(err) => {
                crate::debug::log(
                    "mcp_config_error",
                    serde_json::json!({
                        "path": path.display().to_string(),
                        "error": err.to_string(),
                    }),
                );
            }
        }
    }
}

/// Replace `${VAR_NAME}` in env map values with actual environment variable values.
fn expand_env(env: &mut BTreeMap<String, String>) {
    for val in env.values_mut() {
        if val.starts_with("${") && val.ends_with('}') {
            let var_name = &val[2..val.len() - 1];
            if let Ok(real) = std::env::var(var_name) {
                *val = real;
            }
        }
    }
}
