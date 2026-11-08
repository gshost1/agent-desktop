use std::time::{Duration, Instant};

use super::MenuWaitPhase;

#[test]
fn an_open_read_moves_every_phase_to_open() {
    let now = Instant::now();
    let until = now + Duration::from_secs(1);
    for phase in [
        MenuWaitPhase::AwaitingOpen { until },
        MenuWaitPhase::Open,
        MenuWaitPhase::Closed,
    ] {
        assert_eq!(phase.observe(true, now), MenuWaitPhase::Open);
    }
}

#[test]
fn a_closed_read_inside_the_grace_keeps_awaiting_open() {
    let now = Instant::now();
    let until = now + Duration::from_secs(1);
    let phase = MenuWaitPhase::AwaitingOpen { until };
    assert_eq!(phase.observe(false, now), phase);
}

#[test]
fn a_closed_read_that_began_at_or_after_the_grace_is_closed() {
    let now = Instant::now();
    let phase = MenuWaitPhase::AwaitingOpen { until: now };
    assert_eq!(phase.observe(false, now), MenuWaitPhase::Closed);
}

#[test]
fn a_closed_read_after_open_is_closed_at_once() {
    assert_eq!(
        MenuWaitPhase::Open.observe(false, Instant::now()),
        MenuWaitPhase::Closed
    );
}
