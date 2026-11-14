//! MCP protocol types and JSON-RPC 2.0 wire format.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A JSON-RPC 2.0 request sent from client to server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 notification (has no ID, expects no response).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response received from the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

/// A standard JSON-RPC 2.0 error object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

/// Information identifying this client to an MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

impl Default for McpClientInfo {
    fn default() -> Self {
        Self {
            name: "arqus".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Client capabilities sent during `initialize`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct McpClientCapabilities {}

/// Parameters for the `initialize` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeParams {
    pub protocol_version: String,
    pub capabilities: McpClientCapabilities,
    pub client_info: McpClientInfo,
}

impl Default for McpInitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpClientCapabilities::default(),
            client_info: McpClientInfo::default(),
        }
    }
}

/// Result returned by server from `initialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInitializeResult {
    pub protocol_version: String,
    pub capabilities: Option<Value>,
    pub server_info: Option<McpServerInfo>,
}

/// Information identifying an MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: Option<String>,
}

/// One tool definition returned by `tools/list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_object_schema")]
    pub input_schema: Value,
}

fn default_object_schema() -> Value {
    serde_json::json!({ "type": "object" })
}

/// The result returned by `tools/list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolsListResult {
    pub tools: Vec<McpTool>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

/// Single item in `tools/call` response content array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContent {
    Text {
        text: String,
    },
    Image {
        data: String,
        mime_type: Option<String>,
    },
    Resource {
        resource: Value,
    },
    #[serde(other)]
    Other,
}

/// Result of calling a tool via `tools/call`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCallResult {
    pub content: Vec<McpContent>,
    #[serde(default)]
    pub is_error: bool,
}

/// Configuration for launching one MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub lazy: bool,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: Option<u64>,
    pub approval: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_idle_timeout() -> Option<u64> {
    Some(600)
}
