use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ai::{Llm, LlmRequest};
use crate::flow::config::types::{FlowLimits, OrchestratorConfig, SandboxConfig};
use crate::flow::exec::edits::{apply_edits, parse_edits};
use crate::flow::exec::verify::{check_scope, run_verify_commands};
use crate::flow::model::complete::complete_text;
use crate::flow::model::ledger::{calculate_cost, Ledger, Role, UsageRecord};
use crate::flow::planning::prompts::{
    build_takeover_system_prompt, build_takeover_user_message, RetryFeedback,
};
use crate::flow::repo::context::render_task_context;
use crate::flow::repo::git::reset_hard_clean;
use crate::flow::schema::types::{
    Assignee, FailureKind, FailureRecord, RunState, TaskContract, TaskState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TakeoverRoundResult {
    Accepted { changed_files: Vec<String> },
    Failed(FailureRecord),
}

impl TakeoverRoundResult {
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }
}

pub struct TakeoverContext<'a> {
    pub worktree: &'a Path,
    pub run_state: &'a RunState,
    pub limits: &'a FlowLimits,
    pub sandbox: &'a SandboxConfig,
    pub run_id: &'a str,
}

enum AttemptOutcome {
    Accepted {
        changed_files: Vec<String>,
    },
    Failed {
        record: FailureRecord,
        output: String,
    },
}

async fn run_takeover_attempt(
    llm: &Llm,
    orch_config: &OrchestratorConfig,
    contract: &TaskContract,
    task_state: &TaskState,
    ctx: &TakeoverContext<'_>,
    retry_feedback: Option<&RetryFeedback<'_>>,
    ledger: &mut Option<&mut Ledger>,
) -> AttemptOutcome {
    let attempt = task_state.takeover_attempts;
    let task_context = match render_task_context(ctx.worktree, contract, ctx.run_state, None) {
        Ok(c) => c,
        Err(err) => {
            let record = err.to_failure_record(attempt, Assignee::Orchestrator);
            return AttemptOutcome::Failed {
                record,
                output: String::new(),
            };
        }
    };

    let system_prompt = build_takeover_system_prompt(ctx.limits.whole_file_max_lines);
    let user_msg = build_takeover_user_message(&task_context, &task_state.failures, retry_feedback);
    let req = LlmRequest {
        system: system_prompt,
        messages: vec![user_msg],
        tools: vec![],
        max_tokens: orch_config.max_output_tokens,
    };

    let completion = match complete_text(llm, req).await {
        Ok(c) => c,
        Err(err) => {
            let _ = reset_hard_clean(ctx.worktree);
            let record = err.to_failure_record(attempt, Assignee::Orchestrator);
            return AttemptOutcome::Failed {
                record,
                output: String::new(),
            };
        }
    };

    if let Some(ref mut l) = ledger {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let cost_usd = calculate_cost(
            completion.usage.cost,
            orch_config.price.as_ref(),
            completion.usage.input,
            completion.usage.output,
            completion.usage.cache_read,
        );
        let record = UsageRecord {
            ts,
            run_id: ctx.run_id.to_string(),
            task_id: Some(contract.id.clone()),
            role: Role::Takeover,
            provider: orch_config.provider.clone(),
            model: orch_config.model.clone(),
            input: completion.usage.input,
            output: completion.usage.output,
            cache_read: completion.usage.cache_read,
            cache_write: completion.usage.cache_write,
            estimated: completion.estimated,
            cost_usd,
            latency_ms: completion.latency_ms,
            ok: true,
        };
        let _ = l.record(record);
    }

    let edits = match parse_edits(&completion.text) {
        Ok(e) => e,
        Err(err) => {
            let _ = reset_hard_clean(ctx.worktree);
            let record = err.to_failure_record(attempt, Assignee::Orchestrator);
            return AttemptOutcome::Failed {
                record,
                output: completion.text,
            };
        }
    };

    let changed_files = match apply_edits(&edits, ctx.worktree, &contract.allowed_writes) {
        Ok(files) => files,
        Err(err) => {
            let _ = reset_hard_clean(ctx.worktree);
            let record = err.to_failure_record(attempt, Assignee::Orchestrator);
            return AttemptOutcome::Failed {
                record,
                output: completion.text,
            };
        }
    };

    if let Err(err) = check_scope(
        ctx.worktree,
        &contract.allowed_writes,
        ctx.limits.max_changed_lines_hard,
    ) {
        let _ = reset_hard_clean(ctx.worktree);
        let record = err.to_failure_record(attempt, Assignee::Orchestrator);
        return AttemptOutcome::Failed {
            record,
            output: completion.text,
        };
    }

    let verify_res = match run_verify_commands(
        ctx.worktree,
        &contract.verify,
        &ctx.sandbox.allowed_commands,
        &ctx.sandbox.env_passthrough,
        ctx.limits.verify_timeout_secs,
    )
    .await
    {
        Ok(v) => v,
        Err(err) => {
            let _ = reset_hard_clean(ctx.worktree);
            let record = err.to_failure_record(attempt, Assignee::Orchestrator);
            return AttemptOutcome::Failed {
                record,
                output: completion.text,
            };
        }
    };

    if !verify_res.passed {
        let _ = reset_hard_clean(ctx.worktree);
        let (kind, detail) = if verify_res.timed_out {
            (
                FailureKind::Timeout,
                format!(
                    "verify command timed out: {}",
                    verify_res.failed_command.unwrap_or_default()
                ),
            )
        } else {
            let cmd = verify_res.failed_command.unwrap_or_default();
            let mut det = format!("verify command failed: {cmd}");
            if !verify_res.stderr_tail.is_empty() {
                det.push_str("\nstderr:\n");
                det.push_str(&verify_res.stderr_tail);
            } else if !verify_res.stdout_tail.is_empty() {
                det.push_str("\nstdout:\n");
                det.push_str(&verify_res.stdout_tail);
            }
            (FailureKind::VerifyFailed, det)
        };
        let record = FailureRecord::new(kind, detail, attempt, Assignee::Orchestrator);
        return AttemptOutcome::Failed {
            record,
            output: completion.text,
        };
    }

    AttemptOutcome::Accepted { changed_files }
}

/// Executes up to `limits.max_takeover_attempts` one-shot edit rounds using the orchestrator model.
pub async fn run_takeover_rounds(
    llm: &Llm,
    orch_config: &OrchestratorConfig,
    contract: &TaskContract,
    task_state: &mut TaskState,
    ctx: &TakeoverContext<'_>,
    mut ledger: Option<&mut Ledger>,
) -> TakeoverRoundResult {
    task_state.executor = Assignee::Orchestrator;
    let mut prev_failure: Option<(FailureRecord, String)> = None;

    for attempt in 1..=ctx.limits.max_takeover_attempts {
        task_state.takeover_attempts = attempt;
        let feedback = prev_failure.as_ref().map(|(rec, out)| RetryFeedback {
            previous_output: out,
            failure_kind: rec.kind,
            failure_detail: &rec.detail,
        });

        let outcome = run_takeover_attempt(
            llm,
            orch_config,
            contract,
            task_state,
            ctx,
            feedback.as_ref(),
            &mut ledger,
        )
        .await;

        match outcome {
            AttemptOutcome::Accepted { changed_files } => {
                task_state.changed_files = changed_files.clone();
                return TakeoverRoundResult::Accepted { changed_files };
            }
            AttemptOutcome::Failed { record, output } => {
                if attempt >= ctx.limits.max_takeover_attempts {
                    return TakeoverRoundResult::Failed(record);
                } else {
                    prev_failure = Some((record, output));
                }
            }
        }
    }

    TakeoverRoundResult::Failed(FailureRecord::new(
        FailureKind::NoOutput,
        "max takeover attempts exhausted",
        task_state.takeover_attempts,
        Assignee::Orchestrator,
    ))
}

#[cfg(test)]
#[path = "takeover_tests.rs"]
mod tests;
