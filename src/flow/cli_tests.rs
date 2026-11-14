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
