//! How long the daemon may go without a command, and how often it looks.

use std::time::Duration;

/// The idle limit and the polling tick that never sleeps past it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IdlePolicy {
    pub(crate) idle: Duration,
    pub(crate) tick: Duration,
}

impl IdlePolicy {
    pub(crate) fn new(idle: Duration) -> Self {
        Self {
            idle,
            tick: idle.min(Duration::from_secs(1)),
        }
    }
}
