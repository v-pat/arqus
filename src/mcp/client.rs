//! High-level async MCP client executing JSON-RPC 2.0 requests over a transport.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use super::transport::StdioTransport;
use super::types::{
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpCallResult, McpContent,
    McpInitializeParams, McpInitializeResult, McpTool, McpToolsListResult,
};

pub const DEFAULT_CALL_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_INIT_TIMEOUT_SECS: u64 = 20;

pub struct McpClient {
    transport: StdioTransport,
    next_id: u64,
    pub server_info: Option<String>,
}

impl McpClient {
    /// Launch an MCP server via stdio and complete the initialization handshake.
    pub async fn connect_stdio(
        command: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
        cwd: &Path,
    ) -> Result<Self> {
        let transport = StdioTransport::spawn(command, args, env, cwd)?;
        let mut client = Self {
            transport,
            next_id: 1,
            server_info: None,
        };

        client.initialize(DEFAULT_INIT_TIMEOUT_SECS).await?;
        Ok(client)
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Perform the `initialize` handshake + `notifications/initialized`.
    pub async fn initialize(&mut self, timeout_secs: u64) -> Result<()> {
        let id = self.next_request_id();
        let params = serde_json::to_value(McpInitializeParams::default())?;
        let req = JsonRpcRequest::new(id, "initialize", Some(params));

        let res = self.send_request_with_timeout(&req, timeout_secs).await?;
        if let Some(err) = res.error {
            return Err(anyhow::anyhow!("MCP initialize failed: {err}"));
        }

        let raw_result = res.result.unwrap_or(Value::Null);
        let init_result: McpInitializeResult =
            serde_json::from_value(raw_result).context("invalid initialize response payload")?;

        self.server_info = init_result.server_info.map(|s| {
            format!(
                "{} {}",
                s.name,
                s.version.unwrap_or_else(|| "unknown".to_string())
            )
        });

        // Send required initialized notification.
        let notify = JsonRpcNotification::new("notifications/initialized", None);
        let notify_str = serde_json::to_string(&notify)?;
        self.transport.send_line(&notify_str).await?;

        Ok(())
    }

    /// List all tools exposed by this MCP server.
    pub async fn list_tools(&mut self, timeout_secs: u64) -> Result<Vec<McpTool>> {
        let mut all_tools = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let id = self.next_request_id();
            let params = match &cursor {
                Some(c) => Some(json!({ "cursor": c })),
                None => Some(json!({})),
            };
            let req = JsonRpcRequest::new(id, "tools/list", params);
            let res = self.send_request_with_timeout(&req, timeout_secs).await?;

            if let Some(err) = res.error {
                return Err(anyhow::anyhow!("tools/list failed: {err}"));
            }

            let raw_result = res.result.unwrap_or(Value::Null);
            let list_result: McpToolsListResult = serde_json::from_value(raw_result)
                .context("invalid tools/list response payload")?;

            all_tools.extend(list_result.tools);
            cursor = list_result.next_cursor;
            if cursor.is_none() {
                break;
            }
        }

        Ok(all_tools)
    }

    /// Execute a tool via `tools/call`.
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: &Value,
        timeout_secs: u64,
    ) -> Result<McpCallResult> {
        let id = self.next_request_id();
        let params = json!({
            "name": name,
            "arguments": arguments,
        });
        let req = JsonRpcRequest::new(id, "tools/call", Some(params));
        let res = self.send_request_with_timeout(&req, timeout_secs).await?;

        if let Some(err) = res.error {
            return Ok(McpCallResult {
                content: vec![McpContent::Text {
                    text: err.to_string(),
                }],
                is_error: true,
            });
        }

        let raw_result = res.result.unwrap_or(Value::Null);
        let call_result: McpCallResult =
            serde_json::from_value(raw_result).unwrap_or_else(|err| McpCallResult {
                content: vec![McpContent::Text {
                    text: format!("failed to decode tool output: {err}"),
                }],
                is_error: true,
            });

        Ok(call_result)
    }

    /// Send a request and wait for the response with matching ID, ignoring notifications.
    async fn send_request_with_timeout(
        &mut self,
        req: &JsonRpcRequest,
        timeout_secs: u64,
    ) -> Result<JsonRpcResponse> {
        let req_str = serde_json::to_string(req)?;
        self.transport.send_line(&req_str).await?;

        let duration = Duration::from_secs(timeout_secs);
        let response = tokio::time::timeout(duration, self.read_response_for_id(req.id))
            .await
            .map_err(|_| {
                anyhow::anyhow!("timeout waiting for MCP server response after {timeout_secs}s")
            })??;

        Ok(response)
    }

    async fn read_response_for_id(&mut self, expected_id: u64) -> Result<JsonRpcResponse> {
        loop {
            let line = match self.transport.read_line().await? {
                Some(l) => l,
                None => return Err(anyhow::anyhow!("MCP server closed connection unexpectedly")),
            };

            // Attempt to parse line as a response.
            if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                if resp.id == Some(expected_id) {
                    return Ok(resp);
                }
                // If it's a response with a different ID or a notification, log and keep reading.
                crate::debug::log("mcp_unsolicited", serde_json::json!({ "line": line }));
            }
        }
    }
}
