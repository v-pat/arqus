//! MCP manager coordinating multiple server lifecycles, discovery, and tool dispatch.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ai::ToolDef;
use crate::tools::ToolOutput;

use super::client::McpClient;
use super::config::McpConfig;
use super::schema::{mcp_to_tool_def, parse_tool_name};
use super::types::{McpContent, McpServerConfig, McpTool};

pub const MAX_MCP_OUTPUT_BYTES: usize = 50_000;

#[derive(Clone)]
pub struct McpManager {
    cwd: PathBuf,
    configs: Arc<BTreeMap<String, McpServerConfig>>,
    clients: Arc<Mutex<BTreeMap<String, McpClient>>>,
    cached_tool_defs: Arc<Mutex<Vec<ToolDef>>>,
}

impl std::fmt::Debug for McpManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpManager")
            .field("cwd", &self.cwd)
            .field("configs", &self.configs.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl McpManager {
    pub fn new(cwd: PathBuf, config: McpConfig) -> Self {
        Self {
            cwd,
            configs: Arc::new(config.servers),
            clients: Arc::new(Mutex::new(BTreeMap::new())),
            cached_tool_defs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Load default MCP configuration for cwd.
    pub fn for_cwd(cwd: &Path) -> Self {
        let config = McpConfig::load(cwd);
        Self::new(cwd.to_path_buf(), config)
    }

    /// Returns whether any MCP servers are configured.
    pub fn has_servers(&self) -> bool {
        !self.configs.is_empty()
    }

    /// Discover tools across all configured servers.
    /// Servers are contacted to fetch their `tools/list`.
    pub async fn discover_tools(&self) -> Result<Vec<ToolDef>> {
        let mut defs = Vec::new();

        for (name, conf) in self.configs.as_ref() {
            match self.fetch_server_tools(name, conf).await {
                Ok(tools) => {
                    for tool in tools {
                        defs.push(mcp_to_tool_def(name, &tool));
                    }
                }
                Err(err) => {
                    crate::debug::log(
                        "mcp_discover_error",
                        serde_json::json!({
                            "server": name,
                            "error": err.to_string(),
                        }),
                    );
                }
            }
        }

        // Sort deterministically to maintain prompt KV cache stability.
        defs.sort_by(|a, b| a.name.cmp(&b.name));

        let mut cached = self.cached_tool_defs.lock().await;
        *cached = defs.clone();
        Ok(defs)
    }

    /// Return cached tool definitions, or empty if not yet discovered.
    pub async fn current_tool_defs(&self) -> Vec<ToolDef> {
        self.cached_tool_defs.lock().await.clone()
    }

    /// Dispatch a namespaced tool call (e.g. `mcp__serena__get_symbols`).
    pub async fn execute(&self, namespaced_name: &str, args: &Value) -> ToolOutput {
        let Some((server, tool)) = parse_tool_name(namespaced_name) else {
            return ToolOutput::error(format!("invalid MCP tool name format: `{namespaced_name}`"));
        };

        let conf = match self.configs.get(server) {
            Some(c) => c.clone(),
            None => {
                return ToolOutput::error(format!(
                    "MCP server `{server}` is not configured in mcp.json"
                ))
            }
        };

        match self.call_tool_internal(server, &conf, tool, args).await {
            Ok(output) => output,
            Err(err) => ToolOutput::error(format!("MCP execution error: {err:#}")),
        }
    }

    async fn fetch_server_tools(
        &self,
        server: &str,
        conf: &McpServerConfig,
    ) -> Result<Vec<McpTool>> {
        let mut clients = self.clients.lock().await;
        if !clients.contains_key(server) {
            let client = McpClient::connect_stdio(&conf.command, &conf.args, &conf.env, &self.cwd)
                .await
                .with_context(|| format!("failed connecting to MCP server `{server}`"))?;
            clients.insert(server.to_string(), client);
        }

        let client = clients.get_mut(server).unwrap();
        client.list_tools(30).await
    }

    async fn call_tool_internal(
        &self,
        server: &str,
        conf: &McpServerConfig,
        tool: &str,
        args: &Value,
    ) -> Result<ToolOutput> {
        let mut clients = self.clients.lock().await;
        if !clients.contains_key(server) {
            let client = McpClient::connect_stdio(&conf.command, &conf.args, &conf.env, &self.cwd)
                .await
                .with_context(|| format!("failed connecting to MCP server `{server}`"))?;
            clients.insert(server.to_string(), client);
        }

        let client = clients.get_mut(server).unwrap();
        let timeout_secs = conf.idle_timeout_secs.unwrap_or(60).max(1);
        let result = client.call_tool(tool, args, timeout_secs).await?;

        // Format result text.
        let mut text = String::new();
        for content in result.content {
            match content {
                McpContent::Text { text: t } => {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&t);
                }
                McpContent::Image { mime_type, .. } => {
                    let mime = mime_type.unwrap_or_else(|| "image".to_string());
                    text.push_str(&format!("[Image: {mime}]\n"));
                }
                McpContent::Resource { resource } => {
                    text.push_str(&format!("[Resource: {resource}]\n"));
                }
                McpContent::Other => {}
            }
        }

        // Apply byte capping to protect agent context.
        let is_truncated = text.len() > MAX_MCP_OUTPUT_BYTES;
        let mut truncated = if is_truncated {
            let cut = crate::tools::truncate_bytes(&text, MAX_MCP_OUTPUT_BYTES);
            cut.to_string()
        } else {
            text
        };

        if is_truncated {
            truncated.push_str(&format!(
                "\n\n[output truncated: exceeded {} bytes cap]",
                MAX_MCP_OUTPUT_BYTES
            ));
        }

        if result.is_error {
            Ok(ToolOutput::error(truncated))
        } else {
            Ok(ToolOutput::ok(truncated))
        }
    }

    /// Shutdown all active MCP server child processes.
    pub async fn shutdown(&self) {
        let mut clients = self.clients.lock().await;
        clients.clear();
    }
}
