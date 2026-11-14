use clap::{Args, Subcommand};

#[derive(Debug, Args, Clone, PartialEq)]
pub struct FlowArgs {
    #[command(subcommand)]
    pub command: FlowCommand,
}

#[derive(Debug, Subcommand, Clone, PartialEq)]
pub enum FlowCommand {
    /// Plan and execute tasks collaboratively
    Run(RunArgs),
    /// Resume an existing flow run
    Resume(ResumeArgs),
    /// Print a report for an existing flow run
    Report(ReportArgs),
    /// Live smoke test of all configured flow models
    ModelsCheck,
}

#[derive(Debug, Args, Clone, PartialEq)]
pub struct RunArgs {
    /// Natural language objective to plan and execute
    pub objective: String,
    /// Path to git repository (default: current directory)
    #[arg(long, default_value = ".")]
    pub repo: String,
    /// Override orchestrator model
    #[arg(long)]
    pub orchestrator_model: Option<String>,
    /// Override default worker name
    #[arg(long)]
    pub worker: Option<String>,
    /// Max dollars for cloud orchestrator spend
    #[arg(long)]
    pub max_cost: Option<f64>,
    /// Max cloud tokens for run
    #[arg(long)]
    pub max_cloud_tokens: Option<u64>,
    /// Plan and validate without executing
    #[arg(long)]
    pub dry_plan: bool,
    /// Log full prompts to run directory
    #[arg(long)]
    pub log_prompts: bool,
    /// Append debug JSON lines to debug log
    #[arg(long)]
    pub debug: bool,
}

#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct ResumeArgs {
    /// Run ID to resume
    pub run_id: String,
}

#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct ReportArgs {
    /// Run ID to report on
    pub run_id: String,
}

pub async fn run(_args: &FlowArgs) -> anyhow::Result<()> {
    // Skeleton implementation for F1
    println!("arqus flow skeleton");
    Ok(())
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;

