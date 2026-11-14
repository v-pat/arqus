//! Command-line flags and limit resolution.

use clap::Parser;

use crate::cost::Limits;
use crate::settings::Settings;

const AFTER_HELP: &str = "\
Exit codes:
  0  success
  1  the prompt or run failed (provider error, limit reached, I/O error)
  2  bad configuration or usage (bad flag, unknown provider, missing model)

Startup:
  arqus -c           continue the latest session for this folder
  arqus -r           open the sessions picker
  arqus              start a fresh session

Environment:
  ARQUS_API_KEY              key for every provider (never written to disk)
  ARQUS_PROVIDER             openai, openrouter, anthropic, or a local server
  ARQUS_MODEL                model name
  ARQUS_BASE_URL             endpoint override
  ARQUS_CONTEXT_WINDOW       context size in tokens
  ARQUS_HOME                 global folder; default ~/.arqus
  ARQUS_MAX_OUTPUT_TOKENS    cap for max_tokens on each request
  ARQUS_MAX_RETRIES          retries before a request fails; default 3
  ARQUS_RETRY_BASE_MS        first retry delay in ms; default 1000
  ARQUS_IDLE_TIMEOUT_SECS    abort a stream idle this long; default 60
  ARQUS_DEBUG                log JSON lines to <ARQUS_HOME>/debug.log (1/true)
  ARQUS_APP_TITLE            OpenRouter X-Title header; default arqus
  ARQUS_HTTP_REFERER         OpenRouter HTTP-Referer header

Settings (all keys optional):
  <ARQUS_HOME>/settings.json and <cwd>/.arqus/settings.json (project wins)
  model, base_url, context_window, skill_folders, approval, allow_paths
  compaction: enabled, reserve_tokens, keep_recent_tokens
  limits: max_cost_per_run, max_cost_per_session, max_rounds,
          max_output_tokens, max_tokens_per_run

Slash commands (print mode and TUI unless marked):
  /compact [focus]  summarize old history now
  /cost             show session totals
  /budget           show the OpenRouter key limit
  /tree [id]        list the session tree, or move the leaf (print mode)
  /fork             copy the current path into a new session file
  /model [provider:model]  show or switch the model (TUI)
  /new, /resume, /help, /quit, /exit   (TUI)
  /skill:name [text]  inline a SKILL.md, then the text
  /name [text]        run the prompt template .arqus/prompts/name.md";

#[derive(Debug, Parser)]
#[command(
    name = "arqus",
    version,
    about = "A minimal terminal coding agent",
    after_help = AFTER_HELP
)]
pub struct Cli {
    /// Run one prompt in print mode
    #[arg(short = 'p', long = "prompt", value_name = "PROMPT")]
    pub prompt: Option<String>,
    /// Continue the latest session for this directory
    #[arg(short = 'c', long = "continue")]
    pub continue_last: bool,
    /// Open the sessions picker at startup
    #[arg(short = 'r', long = "resume")]
    pub resume: bool,
    /// Use a temporary session instead of <ARQUS_HOME>/sessions
    #[arg(long = "no-session")]
    pub no_session: bool,
    /// Append debug JSON lines to <ARQUS_HOME>/debug.log
    #[arg(long = "debug")]
    pub debug: bool,
    /// Provider and model, for example anthropic:claude-sonnet-4
    #[arg(long = "model", value_name = "PROVIDER:MODEL")]
    pub model: Option<String>,
    /// Stop when one prompt costs more than this many dollars
    #[arg(long = "max-cost", value_name = "DOLLARS")]
    pub max_cost: Option<f64>,
    /// Stop when the whole session costs more than this many dollars
    #[arg(long = "max-session-cost", value_name = "DOLLARS")]
    pub max_session_cost: Option<f64>,
    /// Maximum model calls in one prompt
    #[arg(long = "max-rounds", value_name = "N")]
    pub max_rounds: Option<usize>,
    /// Maximum generated tokens per request, sent as max_tokens
    #[arg(long = "max-output-tokens", value_name = "N")]
    pub max_output_tokens: Option<u32>,
    /// Fallback token budget for one prompt when the provider reports no cost
    #[arg(long = "max-tokens-per-run", value_name = "N")]
    pub max_tokens_per_run: Option<u64>,
    /// Skip tool approvals (same as approval = "auto")
    #[arg(long = "yolo")]
    pub yolo: bool,
    /// Allow risky bash commands in auto mode (rm -rf /, sudo, ...)
    #[arg(long = "allow-risky")]
    pub allow_risky: bool,
    /// Execution mode: ask (default), auto (same as --yolo), or plan
    #[arg(long = "mode", value_name = "MODE")]
    pub mode: Option<String>,
}

/// Limits: CLI flag beats environment, which beats settings, which beats
/// the built-in defaults.
pub fn resolve_limits(cli: &Cli, settings: &Settings) -> Limits {
    let mut limits = settings.limits(Limits::default());
    if let Ok(text) = std::env::var("ARQUS_MAX_OUTPUT_TOKENS") {
        if let Ok(value) = text.trim().parse::<u32>() {
            if value > 0 {
                limits.max_output_tokens = value;
            }
        }
    }
    if let Some(value) = cli.max_cost {
        limits.max_cost_per_run = value;
    }
    if let Some(value) = cli.max_session_cost {
        limits.max_cost_per_session = value;
    }
    if let Some(value) = cli.max_rounds {
        limits.max_rounds = value;
    }
    if let Some(value) = cli.max_output_tokens {
        limits.max_output_tokens = value;
    }
    if let Some(value) = cli.max_tokens_per_run {
        limits.max_tokens_per_run = value;
    }
    limits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yolo_and_allow_risky_parse() {
        let cli = Cli::try_parse_from(["arqus", "--yolo", "--allow-risky", "-p", "hi"]).unwrap();
        assert!(cli.yolo);
        assert!(cli.allow_risky);
        assert_eq!(cli.prompt.as_deref(), Some("hi"));
        let cli = Cli::try_parse_from(["arqus", "-p", "hi"]).unwrap();
        assert!(!cli.yolo);
        assert!(!cli.allow_risky);
    }

    #[test]
    fn session_start_flags_parse() {
        let cli = Cli::try_parse_from(["arqus", "-c"]).unwrap();
        assert!(cli.continue_last);
        assert!(!cli.resume);
        let cli = Cli::try_parse_from(["arqus", "-r"]).unwrap();
        assert!(cli.resume);
        assert!(!cli.continue_last);
        let cli = Cli::try_parse_from(["arqus"]).unwrap();
        assert!(!cli.continue_last);
        assert!(!cli.resume);
    }

    #[test]
    fn version_and_help_are_complete() {
        use clap::CommandFactory;
        let mut cmd = Cli::command();
        let version = cmd.render_version();
        assert_eq!(
            version.trim(),
            format!("arqus {}", env!("CARGO_PKG_VERSION"))
        );
        let help = cmd.render_help().to_string();
        for needle in [
            "Exit codes:",
            "--max-cost",
            "--yolo",
            "ARQUS_API_KEY",
            "ARQUS_HOME",
            "max_cost_per_run",
            "approval",
            "/compact",
        ] {
            assert!(help.contains(needle), "help is missing {needle}");
        }
    }
}
