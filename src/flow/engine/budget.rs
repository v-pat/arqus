use crate::flow::config::types::FlowLimits;
use crate::flow::errors::FlowError;
use crate::flow::model::ledger::Ledger;

pub fn check_cloud_budget(
    ledger: &Ledger,
    limits: &FlowLimits,
    max_cost: Option<f64>,
) -> Result<(), FlowError> {
    let totals = ledger.totals();
    let cloud = &totals.cloud;
    let cloud_tokens = cloud.input + cloud.output + cloud.cache_read + cloud.cache_write;

    if let Some(cost) = cloud.cost_usd {
        let cost_cap = max_cost.unwrap_or(f64::INFINITY);
        if cost >= cost_cap {
            return Err(FlowError::BudgetExceeded(format!(
                "cost limit reached: ${cost:.2} of ${cost_cap:.2} (limits.max_cost_per_run). Raise it to continue."
            )));
        }
    }

    if cloud_tokens >= limits.max_cloud_tokens_per_run {
        return Err(FlowError::BudgetExceeded(format!(
            "token limit reached: {} of {} (limits.max_cloud_tokens_per_run). Raise it to continue.",
            cloud_tokens, limits.max_cloud_tokens_per_run
        )));
    }

    Ok(())
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
