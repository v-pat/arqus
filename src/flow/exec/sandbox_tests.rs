use std::path::Path;

use super::*;

fn default_allowed() -> Vec<String> {
    vec![
        "cargo test".to_string(),
        "cargo build".to_string(),
        "pytest".to_string(),
        "python3 -m pytest".to_string(),
    ]
}

#[test]
fn test_allowlist_exact_leading_token_match_accepted() {
    let allowed = default_allowed();
    let cwd = Path::new("/tmp");

    let tokens = check_command_allowed("cargo test --lib", &allowed, cwd, None).unwrap();
    assert_eq!(tokens, vec!["cargo", "test", "--lib"]);

    let tokens2 =
        check_command_allowed("python3 -m pytest tests/test_foo.py", &allowed, cwd, None).unwrap();
    assert_eq!(
        tokens2,
        vec!["python3", "-m", "pytest", "tests/test_foo.py"]
    );
}

#[test]
fn test_allowlist_lookalike_command_rejected() {
    let allowed = default_allowed();
    let cwd = Path::new("/tmp");

    let res = check_command_allowed("cargo testx", &allowed, cwd, None);
    assert!(
        res.is_err(),
        "lookalike command 'cargo testx' must be rejected"
    );
}

#[test]
fn test_allowlist_command_with_semicolon_lookalike_rejected() {
    let allowed = default_allowed();
    let cwd = Path::new("/tmp");

    let res = check_command_allowed("pytest; rm -rf /", &allowed, cwd, None);
    assert!(
        res.is_err(),
        "command with semicolon attached to first token 'pytest;' must be rejected"
    );
}

#[test]
fn test_allowlist_empty_command_rejected() {
    let allowed = default_allowed();
    let cwd = Path::new("/tmp");

    let res = check_command_allowed("", &allowed, cwd, None);
    assert!(res.is_err(), "empty command must be rejected");

    let res_ws = check_command_allowed("   ", &allowed, cwd, None);
    assert!(res_ws.is_err(), "whitespace command must be rejected");
}

#[test]
fn test_allowlist_quoted_arguments_split_correctly() {
    let allowed = default_allowed();
    let cwd = Path::new("/tmp");

    let tokens = check_command_allowed(
        "cargo test --test \"e2e flow\" -- --nocapture",
        &allowed,
        cwd,
        None,
    )
    .unwrap();
    assert_eq!(
        tokens,
        vec!["cargo", "test", "--test", "e2e flow", "--", "--nocapture"]
    );
}

#[tokio::test]
async fn test_runner_captures_exit_code_zero_and_nonzero() {
    let cwd = Path::new("/tmp");
    let passthrough = vec!["PATH".to_string()];

    // Success (exit 0)
    let argv_true = vec!["true".to_string()];
    let res_true = run_command(&argv_true, cwd, &passthrough, 10)
        .await
        .unwrap();
    assert_eq!(res_true.exit_code, Some(0));
    assert!(!res_true.timed_out);

    // Non-zero exit is a result, not an error
    let argv_false = vec!["false".to_string()];
    let res_false = run_command(&argv_false, cwd, &passthrough, 10)
        .await
        .unwrap();
    assert_ne!(res_false.exit_code, Some(0));
    assert!(!res_false.timed_out);
}

#[cfg(unix)]
#[tokio::test]
async fn test_runner_timeout_kills_whole_process_group() {
    let cwd = Path::new("/tmp");
    let passthrough = vec!["PATH".to_string()];

    let argv = vec!["sleep".to_string(), "4243".to_string()];
    let res = run_command(&argv, cwd, &passthrough, 1).await.unwrap();

    assert!(res.timed_out);

    // Assert sleep was terminated
    let output = std::process::Command::new("pgrep")
        .args(["-f", "sleep 4243"])
        .output();
    if let Ok(out) = output {
        if out.status.success() {
            assert!(
                out.stdout.is_empty(),
                "child process group must be killed on timeout"
            );
        }
    }
}

#[tokio::test]
async fn test_runner_output_tails_cut_to_4000_chars() {
    let cwd = Path::new("/tmp");
    let passthrough = vec!["PATH".to_string()];

    // python3 -c 'print("x" * 6000)'
    let argv = vec![
        "python3".to_string(),
        "-c".to_string(),
        "print('x' * 6000)".to_string(),
    ];
    let res = run_command(&argv, cwd, &passthrough, 10).await.unwrap();
    assert!(res.stdout.len() <= MAX_OUTPUT_CHARS);
    assert_eq!(res.exit_code, Some(0));
}

#[tokio::test]
async fn test_runner_environment_scrubbed_only_passthrough_retained() {
    let cwd = Path::new("/tmp");
    std::env::set_var("ARQUS_SECRET_TEST_TOKEN", "supersecret123");
    std::env::set_var("ARQUS_PASSTHROUGH_TEST", "safetokeep");

    let passthrough = vec!["PATH".to_string(), "ARQUS_PASSTHROUGH_TEST".to_string()];
    let argv = vec![
        "python3".to_string(),
        "-c".to_string(),
        "import os; print(os.environ.get('ARQUS_SECRET_TEST_TOKEN', 'NONE'), os.environ.get('ARQUS_PASSTHROUGH_TEST', 'NONE'))".to_string(),
    ];

    let res = run_command(&argv, cwd, &passthrough, 10).await.unwrap();
    assert!(
        res.stdout.contains("NONE safetokeep"),
        "secret variable must be scrubbed: got {}",
        res.stdout
    );
}

#[test]
fn test_runner_risky_command_rejected() {
    let allowed = vec!["cargo test".to_string(), "sudo".to_string()];
    let cwd = Path::new("/tmp");

    let res = check_command_allowed("sudo cargo test", &allowed, cwd, None);
    assert!(
        res.is_err(),
        "risky command matching safety patterns must be rejected"
    );
}
