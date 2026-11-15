use std::time::{SystemTime, UNIX_EPOCH};

use crate::ai::{Llm, LlmRequest};
use crate::flow::config::types::OrchestratorConfig;
use crate::flow::model::complete::complete_text;
use crate::flow::model::json::parse_json_object;
use crate::flow::model::ledger::{calculate_cost, Ledger, Role, UsageRecord};
use crate::flow::planning::prompts::{build_redesign_system_prompt, build_redesign_user_message};
use crate::flow::schema::types::{FailureRecord, RedesignDecision, TaskContract};

/// Performs one orchestrator redesign call for a task that exhausted worker attempts.
/// Returns a `RedesignDecision` (Revise, Split, or Takeover).
/// Per spec, any model call failure or unparseable output automatically falls back to Takeover.
pub async fn redesign(
    llm: &Llm,
    orch_config: &OrchestratorConfig,
    contract: &TaskContract,
    failures: &[FailureRecord],
    capability_sheet: &str,
    mut ledger: Option<&mut Ledger>,
    run_id: &str,
) -> RedesignDecision {
    let contract_json = match serde_json::to_string_pretty(contract) {
        Ok(j) => j,
        Err(err) => {
            return RedesignDecision::Takeover {
                reason: format!("failed to serialize task contract: {err}"),
            };
        }
    };

    let system_prompt = build_redesign_system_prompt(capability_sheet);
    let user_msg = build_redesign_user_message(&contract_json, failures);

    let req = LlmRequest {
        system: system_prompt,
        messages: vec![user_msg],
        tools: vec![],
        max_tokens: orch_config.max_output_tokens,
    };

    let completion = match complete_text(llm, req).await {
        Ok(c) => c,
        Err(err) => {
            return RedesignDecision::Takeover {
                reason: format!("redesign model call failed: {err}"),
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
            run_id: run_id.to_string(),
            task_id: Some(contract.id.clone()),
            role: Role::Redesign,
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

    match parse_json_object::<RedesignDecision>(&completion.text) {
        Ok(decision) => decision,
        Err(err) => RedesignDecision::Takeover {
            reason: format!("invalid redesign output: {err}"),
        },
    }
}

#[cfg(test)]
#[path = "redesign_tests.rs"]
mod tests;
