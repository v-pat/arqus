//! Cost and token accounting over session entries.
//!
//! Pure functions only: they read the append-only entry list and never call
//! the network. `Totals` is what `/cost` and the end-of-run line show.

use crate::types::{Entry, EntryKind, Message, Usage};

/// Sum of every request in a set of entries.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Totals {
    pub requests: u64,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    /// None only when no request reported a cost.
    pub cost: Option<f64>,
}

impl Totals {
    fn add_usage(&mut self, usage: &Usage) {
        self.input = self.input.saturating_add(usage.input);
        self.output = self.output.saturating_add(usage.output);
        self.cache_read = self.cache_read.saturating_add(usage.cache_read);
        if let Some(cost) = usage.cost {
            self.cost = Some(self.cost.unwrap_or(0.0) + cost);
        }
    }

    /// Tokens counted for the no-cost fallback limit.
    pub fn tokens(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
    }

    /// Difference between two snapshots, for one run inside a session.
    pub fn since(&self, earlier: &Totals) -> Totals {
        Totals {
            requests: self.requests.saturating_sub(earlier.requests),
            input: self.input.saturating_sub(earlier.input),
            output: self.output.saturating_sub(earlier.output),
            cache_read: self.cache_read.saturating_sub(earlier.cache_read),
            cost: match (self.cost, earlier.cost) {
                (Some(now), Some(before)) => {
                    let delta = now - before;
                    if delta.abs() < 1e-12 {
                        // No request in this run reported a cost.
                        None
                    } else {
                        Some(delta.max(0.0))
                    }
                }
                (Some(now), None) => Some(now),
                _ => None,
            },
        }
    }
}

/// Totals over ALL assistant entries in the file, every branch, plus the
/// usage of compaction summary calls.
pub fn session_totals(entries: &[Entry]) -> Totals {
    let mut totals = Totals::default();
    for entry in entries {
        match &entry.kind {
            EntryKind::Message {
                message: Message::Assistant { usage, .. },
            } => {
                totals.requests = totals.requests.saturating_add(1);
                if let Some(usage) = usage {
                    totals.add_usage(usage);
                }
            }
            EntryKind::Compaction {
                summary_usage: Some(usage),
                ..
            } => {
                totals.requests = totals.requests.saturating_add(1);
                totals.add_usage(usage);
            }
            _ => {}
        }
    }
    totals
}

/// `requests=3 in=1234 out=210 cost=$0.0042`; the cost part is omitted when
/// the provider never reported one.
pub fn totals_line(totals: &Totals) -> String {
    let cost = match totals.cost {
        Some(cost) => format!(" cost=${cost:.4}"),
        None => String::new(),
    };
    format!(
        "requests={} in={} out={}{}",
        totals.requests, totals.input, totals.output, cost
    )
}

/// Print-mode closing line: `[arqus] requests=3 in=1234 out=210 cost=$0.0042 time=12.3s`.
pub fn summary_line(totals: &Totals, seconds: f64) -> String {
    format!("[arqus] {} time={seconds:.1}s", totals_line(totals))
}

/// Hard limits for one run and one session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub max_cost_per_run: f64,
    pub max_cost_per_session: f64,
    pub max_rounds: usize,
    pub max_output_tokens: u32,
    pub max_tokens_per_run: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_cost_per_run: 0.50,
            max_cost_per_session: 5.00,
            max_rounds: 50,
            max_output_tokens: 8192,
            max_tokens_per_run: 500_000,
        }
    }
}

/// Check the limits before one LLM request. `None` means the request may run.
pub fn check_limits(session: &Totals, run: &Totals, limits: &Limits) -> Option<String> {
    if let Some(cost) = run.cost {
        if cost >= limits.max_cost_per_run {
            return Some(format!(
                "cost limit reached: ${cost:.2} of ${:.2} (limits.max_cost_per_run). Raise it to continue.",
                limits.max_cost_per_run
            ));
        }
    } else if run.tokens() >= limits.max_tokens_per_run {
        return Some(format!(
            "token limit reached: {} of {} (limits.max_tokens_per_run). Raise it to continue.",
            run.tokens(),
            limits.max_tokens_per_run
        ));
    }
    if let Some(cost) = session.cost {
        if cost >= limits.max_cost_per_session {
            return Some(format!(
                "cost limit reached: ${cost:.2} of ${:.2} (limits.max_cost_per_session). Raise it to continue.",
                limits.max_cost_per_session
            ));
        }
    }
    None
}

#[cfg(test)]
#[path = "cost_tests.rs"]
mod tests;
