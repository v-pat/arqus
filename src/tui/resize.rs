//! Resize debounce: rapid drags produce one applied size.
//!
//! Pure on purpose: the caller passes elapsed milliseconds from its own clock,
//! so the rule can be tested without sleeping.

/// How long to wait after the last resize event.
pub const DELAY_MS: u64 = 50;

#[derive(Debug, Clone, Default)]
pub struct ResizeDebounce {
    pending: Option<(u16, u16)>,
    /// Absolute deadline in caller milliseconds.
    deadline_ms: Option<u64>,
}

impl ResizeDebounce {
    /// Remember one event, replacing the previous one and restarting the wait.
    pub fn push(&mut self, width: u16, height: u16, now_ms: u64) {
        self.pending = Some((width, height));
        self.deadline_ms = Some(now_ms.saturating_add(DELAY_MS));
    }

    /// Absolute time the caller should wake up, when a change is waiting.
    pub fn deadline_ms(&self) -> Option<u64> {
        self.deadline_ms
    }

    /// Hand back the last size once the quiet window has passed.
    pub fn ready(&mut self, now_ms: u64) -> Option<(u16, u16)> {
        let deadline = self.deadline_ms?;
        if now_ms < deadline {
            return None;
        }
        self.deadline_ms = None;
        self.pending.take()
    }
}

#[cfg(test)]
#[path = "resize_tests.rs"]
mod tests;
