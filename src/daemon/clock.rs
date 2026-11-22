//! Where the daemon reads the time. Tests drive it on a virtual clock instead of waiting for
//! real minutes to pass.

use std::time::Instant;

/// A source of the current instant.
pub(crate) trait Clock {
    fn now(&self) -> Instant;
}

/// The operating system's monotonic clock.
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
