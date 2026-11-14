//! Adversarial and stress tests for MCP server integrations.

use serde_json::json;
use std::collections::BTreeMap;

use super::config::McpConfig;
use super::manager::McpManager;
use super::types::McpServerConfig;

#[tokio::test]
async fn adversarial_serena_server_crash_mid_call() {
    let crash_script = r#"
while read -r line; do
    id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    method=$(echo "$line" | sed -n 's/.*"method":"\([^"]*\)".*/\1/p')
    if [ "$method" = "initialize" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"serverInfo\":{\"name\":\"crashing-serena\",\"version\":\"0.0.1\"}}}"
    elif [ "$method" = "tools/call" ]; then
        exit 1
    fi
done
"#;

    let temp_dir = std::env::temp_dir();
    let mut servers = BTreeMap::new();
    servers.insert(
        "serena".to_string(),
        McpServerConfig {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), crash_script.to_string()],
            env: BTreeMap::new(),
            lazy: true,
            idle_timeout_secs: Some(60),
            approval: None,
        },
    );

    let manager = McpManager::new(temp_dir, McpConfig { servers });
    let output = manager
        .execute(
            "mcp__serena__get_symbols",
            &json!({ "path": "src/main.rs" }),
        )
        .await;

    assert!(output.is_error);
    assert!(output.text.contains("closed connection unexpectedly"));

    manager.shutdown().await;
}

#[tokio::test]
async fn adversarial_serena_server_hang_timeout() {
    let hang_script = r#"
while read -r line; do
    id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    method=$(echo "$line" | sed -n 's/.*"method":"\([^"]*\)".*/\1/p')
    if [ "$method" = "initialize" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"serverInfo\":{\"name\":\"hanging-serena\",\"version\":\"0.0.1\"}}}"
    elif [ "$method" = "tools/call" ]; then
        sleep 100
    fi
done
"#;

    let temp_dir = std::env::temp_dir();
    let mut servers = BTreeMap::new();
    servers.insert(
        "serena".to_string(),
        McpServerConfig {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), hang_script.to_string()],
            env: BTreeMap::new(),
            lazy: true,
            idle_timeout_secs: Some(1),
            approval: None,
        },
    );

    let manager = McpManager::new(temp_dir, McpConfig { servers });
    let output = manager
        .execute(
            "mcp__serena__get_symbols",
            &json!({ "path": "src/main.rs" }),
        )
        .await;

    assert!(output.is_error);
    assert!(output.text.contains("timeout waiting for MCP server"));

    manager.shutdown().await;
}

#[tokio::test]
async fn adversarial_serena_output_bomb_truncation() {
    let bomb_script = r#"
while read -r line; do
    id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    method=$(echo "$line" | sed -n 's/.*"method":"\([^"]*\)".*/\1/p')
    if [ "$method" = "initialize" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"serverInfo\":{\"name\":\"serena\",\"version\":\"0.1.5\"}}}"
    elif [ "$method" = "tools/call" ]; then
        chunk=$(printf 'A%.0s' $(seq 1 200))
        printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"' "$id"
        for i in $(seq 1 1000); do
            printf '%s\\n' "$chunk"
        done
        printf '"}],"isError":false}}\n'
    fi
done
"#;

    let temp_dir = std::env::temp_dir();
    let mut servers = BTreeMap::new();
    servers.insert(
        "serena".to_string(),
        McpServerConfig {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), bomb_script.to_string()],
            env: BTreeMap::new(),
            lazy: true,
            idle_timeout_secs: Some(60),
            approval: None,
        },
    );

    let manager = McpManager::new(temp_dir, McpConfig { servers });
    let output = manager
        .execute(
            "mcp__serena__get_symbols",
            &json!({ "path": "src/main.rs" }),
        )
        .await;

    assert!(!output.is_error);
    assert!(output.text.len() <= 50_500);
    assert!(output
        .text
        .contains("[output truncated: exceeded 50000 bytes cap]"));

    manager.shutdown().await;
}
