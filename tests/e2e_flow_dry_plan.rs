mod common;

use common::*;
use serde_json::json;
use std::fs;
use std::process::Command;

#[test]
fn test_flow_dry_plan_end_to_end() {
    let plan_json = json!({
        "type": "plan",
        "summary": "Dry plan demo",
        "tasks": [{
            "id": "T01",
            "title": "Setup project files",
            "category": "single_function",
            "assignee": "worker",
            "objective": "Add helper in lib.rs",
            "context": [{ "path": "src/main.rs" }],
            "allowed_writes": ["src/lib.rs"],
            "depends_on": [],
            "constraints": [],
            "acceptance_criteria": [],
            "interface_notes": "",
            "verify": ["cargo test"],
            "est_changed_lines": 15
        }],
        "final_verify": ["cargo test"]
    })
    .to_string();

    let plan_chunks = sse_text(&[&plan_json]);
    let server = MockServer::start(vec![Reply::sse(plan_chunks)]);

    let project = TempDir::new("flow_dry_plan_proj");
    let home = TempDir::new("flow_dry_plan_home");

    // Initialize real git repo
    let run_cmd = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&project.path)
            .status()
            .expect("git command failed");
        assert!(status.success(), "git {:?} failed", args);
    };

    run_cmd(&["init"]);
    run_cmd(&["config", "user.name", "Test User"]);
    run_cmd(&["config", "user.email", "test@example.com"]);

    let src_dir = project.path.join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(src_dir.join("main.rs"), "fn main() {}\n").expect("write main.rs");
    fs::write(src_dir.join("lib.rs"), "// lib\n").expect("write lib.rs");

    run_cmd(&["add", "."]);
    run_cmd(&["commit", "-m", "Initial commit"]);

    let settings_dir = project.path.join(".arqus");
    fs::create_dir_all(&settings_dir).expect("create .arqus dir");

    let settings = json!({
        "flow": {
            "orchestrator": {
                "provider": "openai",
                "model": "orch-model",
                "base_url": server.url.clone(),
                "api_key_env": "ARQUS_ORCH_KEY"
            },
            "routing": {
                "default_worker": "worker-1"
            },
            "workers": [
                {
                    "name": "worker-1",
                    "provider": "openai",
                    "model": "worker-model",
                    "base_url": server.url.clone(),
                    "api_key_env": "",
                    "max_prompt_tokens": 4096,
                    "max_output_tokens": 2048,
                    "limits": {
                        "max_context_files": 2,
                        "max_write_files": 1,
                        "max_est_changed_lines": 50
                    }
                }
            ]
        }
    });

    fs::write(
        settings_dir.join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .expect("write settings.json");

    let output = run_arqus(
        &project.path,
        &home.path,
        &server.url,
        &["flow", "run", "Implement helper", "--dry-plan"],
        &[("ARQUS_ORCH_KEY", "test-orch-key")],
    );

    if output.status.code() != Some(0) {
        panic!(
            "dry-plan failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout(&output),
            stderr(&output)
        );
    }

    let out = stdout(&output);
    assert!(out.contains("Plan: Dry plan demo"));
    assert!(out.contains("Tasks (1):"));
    assert!(out.contains("[T01] Setup project files"));
    assert!(out.contains("[arqus flow dry-plan] tasks=1"));

    // Check repository was not touched (no extra commits)
    let git_log = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(&project.path)
        .output()
        .expect("git log");
    let count = String::from_utf8_lossy(&git_log.stdout);
    assert_eq!(count.trim(), "1", "git commit count should remain 1");
}
