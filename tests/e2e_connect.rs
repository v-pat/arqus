//! Black-box tests for the saved key and model files used by the Connect dialog.

mod support;
use support::*;

use std::fs;
use std::process::Command;

fn command_without_arqus_env(cwd: &std::path::Path, home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arqus"));
    for (key, _) in std::env::vars() {
        if key.starts_with("ARQUS_") {
            cmd.env_remove(key);
        }
    }
    cmd.current_dir(cwd).env("ARQUS_HOME", home);
    cmd
}

#[test]
fn print_mode_uses_the_saved_key_and_model() {
    let project = TempDir::new("connect_saved_project");
    let home = TempDir::new("connect_saved_home");
    let fake = FakeLlm::start(vec![Reply::sse(sse_text(&["saved ok"]))]);

    fs::write(
        home.path.join("auth.json"),
        r#"{"openrouter":{"type":"api","key":"saved-key"}}"#,
    )
    .expect("auth file");
    fs::write(
        home.path.join("state.json"),
        r#"{"model":{"provider":"openrouter","id":"saved-model"}}"#,
    )
    .expect("state file");

    let out = command_without_arqus_env(&project.path, &home.path)
        .env("ARQUS_BASE_URL", &fake.url)
        .args(["--no-session", "-p", "hi"])
        .output()
        .expect("run arqus");

    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("saved ok"));
    let requests = fake.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].header("authorization"),
        Some("Bearer saved-key")
    );
    assert_eq!(requests[0].json()["model"], "saved-model");
}

#[test]
fn no_provider_prints_the_connect_hint_and_exits_2() {
    let project = TempDir::new("connect_none_project");
    let home = TempDir::new("connect_none_home");

    let out = command_without_arqus_env(&project.path, &home.path)
        .args(["--no-session", "-p", "hi"])
        .output()
        .expect("run arqus");

    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("No provider connected. Run arqus and use /connect"),
        "stderr: {}",
        stderr(&out)
    );
}
