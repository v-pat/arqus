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
