//! When an idle daemon exits: the instant of its last command against its `IdlePolicy`.

use std::time::{Duration, Instant};

use super::idle_policy::IdlePolicy;

/// The instant of the last command, measured against an `IdlePolicy`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IdleTimer {
    idle: Duration,
    last_activity: Instant,
}

impl IdleTimer {
    pub(crate) fn start(policy: IdlePolicy, now: Instant) -> Self {
        Self {
            idle: policy.idle,
            last_activity: now,
        }
    }

    pub(crate) fn touch(&mut self, now: Instant) {
        self.last_activity = now;
    }

    pub(crate) fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last_activity) >= self.idle
    }
}

#[cfg(test)]
#[path = "idle_timer_tests.rs"]
mod tests;
