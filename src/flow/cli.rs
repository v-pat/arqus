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

pub async fn run(args: &FlowArgs) -> anyhow::Result<()> {
    match &args.command {
        FlowCommand::Run(_run_args) => {
            println!("arqus flow run");
            Ok(())
        }
        FlowCommand::Resume(_resume_args) => {
            println!("arqus flow resume");
            Ok(())
        }
        FlowCommand::Report(_report_args) => {
            println!("arqus flow report");
            Ok(())
        }
        FlowCommand::ModelsCheck => models_check().await,
    }
}

async fn models_check() -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let overrides = crate::flow::config::FlowCliOverrides::default();
    let config = crate::flow::config::load_flow_config(&home, &cwd, &overrides)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut failed = false;

    let orch_cfg = crate::ai::ModelConfig {
        provider: config.orchestrator.provider.clone(),
        model: config.orchestrator.model.clone(),
        base_url: config.orchestrator.base_url.clone(),
        api_key: config.orchestrator_key.clone(),
        context_window: crate::ai::default_context_window(&config.orchestrator.provider),
        extra_body: config.orchestrator.extra_body.clone(),
    };
    let orch_llm = crate::ai::Llm::from_config(orch_cfg.clone());
    let ping_req = crate::ai::LlmRequest {
        system: "You are a test helper.".to_string(),
        messages: vec![crate::types::Message::User {
            text: "ping".to_string(),
        }],
        tools: vec![],
        max_tokens: Some(16),
    };

    println!(
        "Checking orchestrator {} ({}) at {}...",
        orch_cfg.model, orch_cfg.provider, orch_cfg.base_url
    );
    match crate::flow::model::complete::complete_text(&orch_llm, ping_req.clone()).await {
        Ok(res) => {
            println!(
                "[orchestrator] {}/{}: ok (latency: {}ms)",
                orch_cfg.provider, orch_cfg.model, res.latency_ms
            );
        }
        Err(err) => {
            println!(
                "[orchestrator] {}/{}: FAILED - {}",
                orch_cfg.provider, orch_cfg.model, err
            );
            failed = true;
        }
    }

    for worker in &config.workers {
        let worker_key = config
            .worker_keys
            .get(&worker.name)
            .cloned()
            .unwrap_or_else(|| "local".to_string());
        let worker_cfg = crate::ai::ModelConfig {
            provider: worker.provider.clone(),
            model: worker.model.clone(),
            base_url: worker.base_url.clone(),
            api_key: worker_key,
            context_window: worker.max_prompt_tokens as u64,
            extra_body: worker.extra_body.clone(),
        };
        let worker_llm = crate::ai::Llm::from_config(worker_cfg.clone());
        println!(
            "Checking worker '{}' {} ({}) at {}...",
            worker.name, worker_cfg.model, worker_cfg.provider, worker_cfg.base_url
        );
        match crate::flow::model::complete::complete_text(&worker_llm, ping_req.clone()).await {
            Ok(res) => {
                println!(
                    "[worker: {}] {}/{}: ok (latency: {}ms)",
                    worker.name, worker_cfg.provider, worker_cfg.model, res.latency_ms
                );
            }
            Err(err) => {
                println!(
                    "[worker: {}] {}/{}: FAILED - {}",
                    worker.name, worker_cfg.provider, worker_cfg.model, err
                );
                failed = true;
            }
        }
    }

    if failed {
        anyhow::bail!("one or more models failed check");
    }
    Ok(())
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
