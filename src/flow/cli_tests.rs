use super::*;
use crate::cli::{Cli, Command};
use clap::Parser;

#[test]
fn test_flow_subcommand_help_parses() {
    let cli = Cli::try_parse_from(["arqus", "flow", "models-check"]).unwrap();
    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::ModelsCheck,
        })) => {}
        _ => panic!("Expected ModelsCheck subcommand"),
    }
}

#[test]
fn test_flow_run_subcommand_args() {
    let cli = Cli::try_parse_from([
        "arqus",
        "flow",
        "run",
        "build feature X",
        "--repo",
        "./sub",
        "--dry-plan",
        "--max-cost",
        "1.50",
    ])
    .unwrap();

    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::Run(args),
        })) => {
            assert_eq!(args.objective, "build feature X");
            assert_eq!(args.repo, "./sub");
            assert!(args.dry_plan);
            assert_eq!(args.max_cost, Some(1.50));
        }
        _ => panic!("Expected Run subcommand"),
    }
}

#[test]
fn test_existing_cli_without_subcommand_unchanged() {
    let cli = Cli::try_parse_from(["arqus", "-p", "hello world"]).unwrap();
    assert_eq!(cli.prompt.as_deref(), Some("hello world"));
    assert!(cli.command.is_none());

    let cli = Cli::try_parse_from(["arqus", "-c"]).unwrap();
    assert!(cli.continue_last);
    assert!(cli.command.is_none());
}

#[test]
fn test_dry_plan_flag_parsed() {
    let cli = Cli::try_parse_from(["arqus", "flow", "run", "do task", "--dry-plan"]).unwrap();
    if let Some(Command::Flow(FlowArgs {
        command: FlowCommand::Run(args),
    })) = cli.command
    {
        assert!(args.dry_plan);
        assert_eq!(args.objective, "do task");
    } else {
        panic!("expected Run with dry_plan");
    }
}

#[test]
fn test_flow_resume_subcommand_parses() {
    let cli = Cli::try_parse_from(["arqus", "flow", "resume", "abc12345"]).unwrap();
    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::Resume(args),
        })) => {
            assert_eq!(args.run_id, "abc12345");
        }
        _ => panic!("Expected Resume subcommand"),
    }
}

#[test]
fn test_flow_report_subcommand_parses() {
    let cli = Cli::try_parse_from(["arqus", "flow", "report", "run-987"]).unwrap();
    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::Report(args),
        })) => {
            assert_eq!(args.run_id, "run-987");
        }
        _ => panic!("Expected Report subcommand"),
    }
}

#[test]
fn test_cli_parse_flow_run_all_options() {
    let cli = Cli::try_parse_from([
        "arqus",
        "flow",
        "run",
        "build complete system",
        "--repo",
        "/custom/repo",
        "--orchestrator-model",
        "custom-orch",
        "--worker",
        "custom-worker",
        "--max-cost",
        "2.75",
        "--max-cloud-tokens",
        "150000",
        "--dry-plan",
        "--log-prompts",
        "--debug",
    ])
    .unwrap();

    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::Run(args),
        })) => {
            assert_eq!(args.objective, "build complete system");
            assert_eq!(args.repo, "/custom/repo");
            assert_eq!(args.orchestrator_model.as_deref(), Some("custom-orch"));
            assert_eq!(args.worker.as_deref(), Some("custom-worker"));
            assert_eq!(args.max_cost, Some(2.75));
            assert_eq!(args.max_cloud_tokens, Some(150000));
            assert!(args.dry_plan);
            assert!(args.log_prompts);
            assert!(args.debug);
        }
        _ => panic!("Expected Run subcommand with all options"),
    }
}

#[test]
fn test_cli_parse_flow_resume_with_repo() {
    let cli = Cli::try_parse_from([
        "arqus",
        "flow",
        "resume",
        "run-resume-123",
        "--repo",
        "/path/to/repo",
    ])
    .unwrap();
    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::Resume(args),
        })) => {
            assert_eq!(args.run_id, "run-resume-123");
            assert_eq!(args.repo, "/path/to/repo");
        }
        _ => panic!("Expected Resume subcommand"),
    }
}

#[test]
fn test_cli_parse_flow_report_with_repo() {
    let cli = Cli::try_parse_from([
        "arqus",
        "flow",
        "report",
        "run-report-123",
        "--repo",
        "/path/to/repo",
    ])
    .unwrap();
    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::Report(args),
        })) => {
            assert_eq!(args.run_id, "run-report-123");
            assert_eq!(args.repo, "/path/to/repo");
        }
        _ => panic!("Expected Report subcommand"),
    }
}

#[test]
fn test_docker_wrapper_forwards_role_keys() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let script_path = std::path::Path::new(manifest_dir)
        .join("scripts")
        .join("arqus-docker");
    let content = std::fs::read_to_string(&script_path).expect("read arqus-docker script");
    assert!(
        content.contains("ARQUS_ORCHESTRATOR_KEY"),
        "arqus-docker must forward ARQUS_ORCHESTRATOR_KEY"
    );
    assert!(
        content.contains("ARQUS_WORKER_KEY"),
        "arqus-docker must forward ARQUS_WORKER_KEY"
    );
}
