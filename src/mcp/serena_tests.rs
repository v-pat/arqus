//! Functional tests for Serena MCP server support in arqus.

use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::agent::safety_flow::{approve, preview, PLAN_REFUSAL};
use crate::agent::{run_prompt, AgentConfig};
use crate::ai::{Llm, StreamEvent};
use crate::safety::{Approvals, Approver, Guard};
use crate::session::Session;

use super::config::McpConfig;
use super::manager::McpManager;
use super::types::McpServerConfig;

/// Build a mock Serena MCP server shell script that responds to standard MCP protocol requests.
pub fn serena_mock_script() -> String {
    r#"
while read -r line; do
    id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    method=$(echo "$line" | sed -n 's/.*"method":"\([^"]*\)".*/\1/p')

    if [ "$method" = "initialize" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{\"listChanged\":true}},\"serverInfo\":{\"name\":\"serena\",\"version\":\"0.1.5\"}}}"
    elif [ "$method" = "tools/list" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"get_symbols\",\"description\":\"Retrieve symbol AST for a source file in the project.\",\"inputSchema\":{\"\$schema\":\"http://json-schema.org/draft-07/schema#\",\"title\":\"GetSymbolsInput\",\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"}},\"required\":[\"path\"]}},{\"name\":\"edit_symbol\",\"description\":\"Perform semantic symbol modification.\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"path\":{\"type\":\"string\"},\"symbol_name\":{\"type\":\"string\"},\"new_code\":{\"type\":\"string\"}},\"required\":[\"path\",\"symbol_name\",\"new_code\"]}}]}}"
    elif [ "$method" = "tools/call" ]; then
        tool=$(echo "$line" | sed -n 's/.*"name":"\([^"]*\)".*/\1/p')
        if [ "$tool" = "get_symbols" ]; then
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"SymbolTree: fn main() -> () at lines 1-10; struct App at lines 12-30\"}],\"isError\":false}}"
        elif [ "$tool" = "edit_symbol" ]; then
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"Symbol App updated successfully.\"}],\"isError\":false}}"
        else
            echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"Unknown serena tool\"}],\"isError\":true}}"
        fi
    fi
done
"#.to_string()
}

fn mock_llm(script: Vec<Vec<StreamEvent>>) -> Llm {
    Llm::Mock(Arc::new(Mutex::new(VecDeque::from(script))))
}

#[tokio::test]
async fn serena_discovery_and_schema_pruning() {
    let script = serena_mock_script();
    let temp_dir = std::env::temp_dir();

    let mut servers = BTreeMap::new();
    servers.insert(
        "serena".to_string(),
        McpServerConfig {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), script],
            env: BTreeMap::new(),
            lazy: true,
            idle_timeout_secs: Some(60),
            approval: Some("ask".to_string()),
        },
    );

    let manager = McpManager::new(temp_dir.clone(), McpConfig { servers });
    let tool_defs = manager
        .discover_tools()
        .await
        .expect("discover serena tools");

    assert_eq!(tool_defs.len(), 2);
    let names: Vec<_> = tool_defs.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["mcp__serena__edit_symbol", "mcp__serena__get_symbols"]
    );

    let get_syms = tool_defs
        .iter()
        .find(|t| t.name == "mcp__serena__get_symbols")
        .unwrap();
    let params = &get_syms.parameters;
    assert!(!params.as_object().unwrap().contains_key("$schema"));
    assert!(!params.as_object().unwrap().contains_key("title"));
    assert_eq!(params["properties"]["path"]["type"], "string");

    manager.shutdown().await;
}

#[tokio::test]
async fn serena_tool_execution_success() {
    let script = serena_mock_script();
    let temp_dir = std::env::temp_dir();

    let mut servers = BTreeMap::new();
    servers.insert(
        "serena".to_string(),
        McpServerConfig {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), script],
            env: BTreeMap::new(),
            lazy: true,
            idle_timeout_secs: Some(60),
            approval: Some("ask".to_string()),
        },
    );

    let manager = McpManager::new(temp_dir.clone(), McpConfig { servers });
    let output = manager
        .execute(
            "mcp__serena__get_symbols",
            &json!({ "path": "src/main.rs" }),
        )
        .await;

    assert!(!output.is_error);
    assert!(output.text.contains("SymbolTree: fn main()"));

    manager.shutdown().await;
}

#[tokio::test]
async fn serena_safety_and_plan_mode() {
    let (events_tx, _events_rx) = mpsc::channel(10);
    let (_approval_tx, mut approval_rx) = mpsc::channel(1);

    let cfg_plan = AgentConfig {
        cwd: std::env::temp_dir(),
        system_prompt: String::new(),
        context_window: 128000,
        compaction: Default::default(),
        limits: crate::cost::Limits::default(),
        approver: Approver::Auto,
        allow_risky: false,
        plan: true,
        approvals: Approvals::default(),
        guard: Guard::default(),
        mcp: None,
    };

    // Read-only Serena tool (get_symbols) is allowed in plan mode
    let res_read = approve(
        "mcp__serena__get_symbols",
        &json!({ "path": "src/main.rs" }),
        &cfg_plan,
        &events_tx,
        &mut approval_rx,
    )
    .await;
    assert!(res_read.is_ok());

    // Mutating Serena tool (edit_symbol) is refused in plan mode
    let res_edit = approve(
        "mcp__serena__edit_symbol",
        &json!({ "path": "src/main.rs", "symbol_name": "App", "new_code": "" }),
        &cfg_plan,
        &events_tx,
        &mut approval_rx,
    )
    .await;
    assert!(res_edit.is_err());
    assert_eq!(res_edit.unwrap_err().text, PLAN_REFUSAL);
}

#[test]
fn serena_approval_preview_formatting() {
    let args = json!({
        "path": "src/agent.rs",
        "symbol_name": "AgentConfig",
        "new_code": "pub struct AgentConfig { ... }"
    });

    let text = preview("mcp__serena__edit_symbol", &args, None);
    assert!(text.contains("mcp server: serena"));
    assert!(text.contains("mcp tool:   edit_symbol"));
    assert!(text.contains("arguments:"));
    assert!(text.contains("\"path\": \"src/agent.rs\""));
}

#[tokio::test]
async fn full_agent_turn_with_serena_mcp() {
    let script = serena_mock_script();
    let temp_dir = std::env::temp_dir().join(format!("serena_turn_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut servers = BTreeMap::new();
    servers.insert(
        "serena".to_string(),
        McpServerConfig {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), script],
            env: BTreeMap::new(),
            lazy: true,
            idle_timeout_secs: Some(60),
            approval: Some("auto".to_string()),
        },
    );

    let mcp = McpManager::new(temp_dir.clone(), McpConfig { servers });
    let _ = mcp.discover_tools().await.unwrap();

    let cfg = AgentConfig {
        cwd: temp_dir.clone(),
        system_prompt: "test".to_string(),
        context_window: 128000,
        compaction: Default::default(),
        limits: crate::cost::Limits::default(),
        approver: Approver::Auto,
        allow_risky: false,
        plan: false,
        approvals: Approvals::default(),
        guard: Guard::default(),
        mcp: Some(mcp.clone()),
    };

    let stream_events = vec![
        vec![
            StreamEvent::ToolCall {
                id: "call_1".to_string(),
                name: "mcp__serena__get_symbols".to_string(),
                args: json!({ "path": "src/main.rs" }),
            },
            StreamEvent::Done(crate::types::Message::Assistant {
                blocks: vec![crate::types::Block::ToolCall {
                    id: "call_1".to_string(),
                    name: "mcp__serena__get_symbols".to_string(),
                    args: json!({ "path": "src/main.rs" }),
                }],
                provider: "mock".to_string(),
                model: "mock-model".to_string(),
                usage: None,
                stop: crate::types::StopReason::ToolUse,
            }),
        ],
        vec![
            StreamEvent::TextDelta("Here is the symbol analysis.".to_string()),
            StreamEvent::Done(crate::types::Message::Assistant {
                blocks: vec![crate::types::Block::Text {
                    text: "Here is the symbol analysis.".to_string(),
                }],
                provider: "mock".to_string(),
                model: "mock-model".to_string(),
                usage: None,
                stop: crate::types::StopReason::Stop,
            }),
        ],
    ];

    let llm = mock_llm(stream_events);
    let mut session = Session::create(&temp_dir, "/x").unwrap();
    let (events_tx, _events_rx) = mpsc::channel(32);
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let (_approval_tx, approval_rx) = mpsc::channel(1);

    run_prompt(
        &mut session,
        &llm,
        &cfg,
        "Analyze the symbols in main.rs".to_string(),
        events_tx,
        steer_rx,
        approval_rx,
    )
    .await
    .expect("agent prompt with serena mcp to succeed");

    let entries = &session.entries;
    assert_eq!(entries.len(), 4);

    if let crate::types::EntryKind::Message {
        message:
            crate::types::Message::ToolResult {
                tool_name,
                text,
                is_error,
                ..
            },
    } = &entries[2].kind
    {
        assert_eq!(tool_name, "mcp__serena__get_symbols");
        assert!(!(*is_error));
        assert!(text.contains("SymbolTree: fn main()"));
    } else {
        panic!("entry 2 should be ToolResult for serena");
    }

    mcp.shutdown().await;
    let _ = std::fs::remove_dir_all(&temp_dir);
}
