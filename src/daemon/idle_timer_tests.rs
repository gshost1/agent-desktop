use super::IdleTimer;
use crate::daemon::clock::Clock;
use crate::daemon::idle_policy::IdlePolicy;
use std::cell::Cell;
use std::time::{Duration, Instant};

/// A clock that moves only when the test advances it.
pub(crate) struct VirtualClock(Cell<Instant>);

impl VirtualClock {
    pub(crate) fn new() -> Self {
        Self(Cell::new(Instant::now()))
    }

    pub(crate) fn advance(&self, by: Duration) {
        self.0.set(self.0.get() + by);
    }
}

impl Clock for VirtualClock {
    fn now(&self) -> Instant {
        self.0.get()
    }
}

#[test]
fn timer_expires_exactly_at_the_idle_limit() {
    let clock = VirtualClock::new();
    let timer = IdleTimer::start(IdlePolicy::new(Duration::from_secs(300)), clock.now());
    clock.advance(Duration::from_secs(299));
    assert!(!timer.expired(clock.now()));
    clock.advance(Duration::from_secs(1));

    assert!(timer.expired(clock.now()));
}

#[test]
fn a_command_restarts_the_idle_limit() {
    let clock = VirtualClock::new();
    let mut timer = IdleTimer::start(IdlePolicy::new(Duration::from_secs(300)), clock.now());
    clock.advance(Duration::from_secs(200));
    timer.touch(clock.now());
    clock.advance(Duration::from_secs(200));

    assert!(!timer.expired(clock.now()));
}

#[test]
fn the_daemon_looks_at_least_once_a_second_and_never_sleeps_past_its_limit() {
    assert_eq!(
        IdlePolicy::new(Duration::from_secs(300)).tick,
        Duration::from_secs(1)
    );
    assert_eq!(
        IdlePolicy::new(Duration::from_millis(200)).tick,
        Duration::from_millis(200)
    );
}
