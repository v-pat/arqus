//! Unit tests for MCP protocol, schema pruning, config, and client interactions.

use serde_json::json;
use std::collections::BTreeMap;

use super::client::McpClient;
use super::config::{McpConfig, McpConfigFile};
use super::schema::{clean_description, format_tool_name, parse_tool_name, prune_schema};
use super::types::{JsonRpcRequest, JsonRpcResponse, McpContent};

#[test]
fn jsonrpc_request_and_response_roundtrip() {
    let req = JsonRpcRequest::new(42, "tools/list", Some(json!({ "cursor": "abc" })));
    let req_str = serde_json::to_string(&req).unwrap();
    assert!(req_str.contains("\"jsonrpc\":\"2.0\""));
    assert!(req_str.contains("\"id\":42"));
    assert!(req_str.contains("\"method\":\"tools/list\""));

    let res_json = r#"{"jsonrpc":"2.0","id":42,"result":{"tools":[]}}"#;
    let res: JsonRpcResponse = serde_json::from_str(res_json).unwrap();
    assert_eq!(res.id, Some(42));
    assert!(res.error.is_none());
    assert!(res.result.is_some());
}

#[test]
fn jsonrpc_error_deserialization() {
    let err_json =
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
    let res: JsonRpcResponse = serde_json::from_str(err_json).unwrap();
    assert_eq!(res.id, Some(1));
    let err = res.error.unwrap();
    assert_eq!(err.code, -32601);
    assert_eq!(err.message, "Method not found");
}

#[test]
fn tool_name_formatting_and_parsing() {
    let formatted = format_tool_name("serena", "find_symbols");
    assert_eq!(formatted, "mcp__serena__find_symbols");

    let parsed = parse_tool_name(&formatted);
    assert_eq!(parsed, Some(("serena", "find_symbols")));

    // Invalid formats
    assert_eq!(parse_tool_name("bash"), None);
    assert_eq!(parse_tool_name("mcp____"), None);
    assert_eq!(parse_tool_name("mcp__serena__"), None);
    assert_eq!(parse_tool_name("other__serena__tool"), None);
}

#[test]
fn schema_pruning_strips_metadata() {
    let verbose_schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "QueryParameters",
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "title": "Search Query",
                "description": "The natural language search query to execute against the codebase. This will search across all symbols.",
                "examples": ["find user auth"]
            },
            "limit": {
                "type": "integer",
                "default": null
            }
        },
        "required": ["query"]
    });

    let pruned = prune_schema(&verbose_schema);
    assert!(!pruned.as_object().unwrap().contains_key("$schema"));
    assert!(!pruned.as_object().unwrap().contains_key("title"));

    let query_prop = &pruned["properties"]["query"];
    assert!(!query_prop.as_object().unwrap().contains_key("title"));
    assert!(!query_prop.as_object().unwrap().contains_key("examples"));
    assert_eq!(query_prop["type"], "string");

    let desc = query_prop["description"].as_str().unwrap();
    assert!(desc.starts_with("The natural language search query"));
}

#[test]
fn clean_description_caps_and_sentences() {
    let short = "Find all symbol definitions in the current workspace.";
    assert_eq!(
        clean_description(short),
        "Find all symbol definitions in the current workspace."
    );

    let multi_sentence = "First sentence. Second sentence with extra fluff.";
    assert_eq!(clean_description(multi_sentence), "First sentence.");

    let long_word = "a".repeat(300);
    let cleaned = clean_description(&long_word);
    assert!(cleaned.len() <= 205);
    assert!(cleaned.ends_with("..."));
}

#[test]
fn config_parsing_and_env_expansion() {
    let json_text = r#"{
        "mcpServers": {
            "test_server": {
                "command": "python3",
                "args": ["-m", "test_mcp"],
                "env": {
                    "TEST_VAR": "${PATH}"
                },
                "lazy": true
            }
        }
    }"#;

    let parsed: McpConfigFile = serde_json::from_str(json_text).unwrap();
    assert!(parsed.mcp_servers.contains_key("test_server"));

    let temp_dir = std::env::temp_dir().join(format!("arqus_mcp_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let config_path = temp_dir.join("mcp.json");
    std::fs::write(&config_path, json_text).unwrap();

    let loaded = McpConfig::load_in(&temp_dir, &temp_dir);
    assert!(loaded.servers.contains_key("test_server"));
    let s = &loaded.servers["test_server"];
    assert_eq!(s.command, "python3");
    // Verify PATH was expanded
    assert!(!s.env["TEST_VAR"].is_empty());
    assert_ne!(s.env["TEST_VAR"], "${PATH}");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn mock_mcp_server_handshake_and_tool_call() {
    // A mock server script in sh that responds to JSON-RPC over stdin/stdout.
    let script = r#"
while read -r line; do
    id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    method=$(echo "$line" | sed -n 's/.*"method":"\([^"]*\)".*/\1/p')
    if [ "$method" = "initialize" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"serverInfo\":{\"name\":\"mock-server\",\"version\":\"1.0.0\"}}}"
    elif [ "$method" = "tools/list" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"echo\",\"description\":\"Echo text.\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"msg\":{\"type\":\"string\"}},\"required\":[\"msg\"]}}]}}"
    elif [ "$method" = "tools/call" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"echoed successfully\"}],\"isError\":false}}"
    fi
done
"#;

    let cwd = std::env::temp_dir();
    let mut client = McpClient::connect_stdio(
        "sh",
        &["-c".to_string(), script.to_string()],
        &BTreeMap::new(),
        &cwd,
    )
    .await
    .expect("connect to mock MCP server");

    assert_eq!(client.server_info.as_deref(), Some("mock-server 1.0.0"));

    let tools = client.list_tools(5).await.expect("list_tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");

    let call_res = client
        .call_tool("echo", &json!({ "msg": "hello" }), 5)
        .await
        .expect("call_tool");
    assert!(!call_res.is_error);
    assert_eq!(call_res.content.len(), 1);
    match &call_res.content[0] {
        McpContent::Text { text } => assert_eq!(text, "echoed successfully"),
        _ => panic!("expected text content"),
    }
}
