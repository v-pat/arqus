mod common;

use common::*;
use serde_json::json;

#[test]
fn models_check_runs_smoke_tests_against_configured_models_with_extra_body() {
    let orchestrator_chunks = sse_text(&["ok orchestrator"]);
    let worker_chunks = sse_text(&["ok worker"]);

    let orchestrator_server = MockServer::start(vec![Reply::sse(orchestrator_chunks)]);
    let worker_server = MockServer::start(vec![Reply::sse(worker_chunks)]);

    let project = TempDir::new("flow_models_check_proj");
    let home = TempDir::new("flow_models_check_home");
    let settings_dir = project.path.join(".arqus");
    std::fs::create_dir_all(&settings_dir).expect("create .arqus dir");

    let settings = json!({
        "flow": {
            "orchestrator": {
                "provider": "openai",
                "model": "orch-model",
                "base_url": orchestrator_server.url.clone(),
                "api_key_env": "ARQUS_ORCH_KEY",
                "extra_body": {
                    "orch_custom": "orch_value"
                }
            },
            "routing": {
                "default_worker": "worker-1"
            },
            "workers": [
                {
                    "name": "worker-1",
                    "provider": "openai",
                    "model": "worker-model",
                    "base_url": worker_server.url.clone(),
                    "api_key_env": "",
                    "max_prompt_tokens": 4096,
                    "max_output_tokens": 2048,
                    "extra_body": {
                        "worker_custom": "worker_value",
                        "temperature": 0.1
                    },
                    "limits": {
                        "max_context_files": 2,
                        "max_write_files": 1,
                        "max_est_changed_lines": 50
                    }
                }
            ]
        }
    });

    std::fs::write(
        settings_dir.join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .expect("write settings.json");

    let output = run_arqus(
        &project.path,
        &home.path,
        &orchestrator_server.url,
        &["flow", "models-check"],
        &[("ARQUS_ORCH_KEY", "orch_secret")],
    );

    if output.status.code() != Some(0) {
        panic!(
            "models-check failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout(&output),
            stderr(&output)
        );
    }
    assert_eq!(output.status.code(), Some(0));
    let out = stdout(&output);
    assert!(out.contains("orch-model") || out.contains("orchestrator"));
    assert!(out.contains("worker-1") || out.contains("worker-model"));

    let orch_reqs = orchestrator_server.requests();
    assert_eq!(orch_reqs.len(), 1);
    let orch_body = orch_reqs[0].json();
    assert_eq!(orch_body["model"], "orch-model");
    assert_eq!(orch_body["orch_custom"], "orch_value");

    let worker_reqs = worker_server.requests();
    assert_eq!(worker_reqs.len(), 1);
    let worker_body = worker_reqs[0].json();
    assert_eq!(worker_body["model"], "worker-model");
    assert_eq!(worker_body["worker_custom"], "worker_value");
    assert_eq!(worker_body["temperature"], 0.1);
}
