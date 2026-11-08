use std::time::{Duration, Instant};

use super::MenuWaitPhase;

fn awaiting(until: Instant, read_closed: bool) -> MenuWaitPhase {
    MenuWaitPhase::AwaitingOpen { until, read_closed }
}

#[test]
fn an_open_read_moves_every_phase_to_open() {
    let now = Instant::now();
    let until = now + Duration::from_secs(1);
    for phase in [
        awaiting(until, false),
        MenuWaitPhase::Open,
        MenuWaitPhase::Closing,
        MenuWaitPhase::Closed,
    ] {
        assert_eq!(phase.observe(true, now), MenuWaitPhase::Open);
    }
}

#[test]
fn a_closed_read_inside_the_grace_keeps_awaiting_open_and_records_the_read() {
    let now = Instant::now();
    let until = now + Duration::from_secs(1);
    assert_eq!(
        awaiting(until, false).observe(false, now),
        awaiting(until, true)
    );
}

#[test]
fn a_closed_read_that_began_at_or_after_the_grace_is_closed() {
    let now = Instant::now();
    assert_eq!(
        awaiting(now, false).observe(false, now),
        MenuWaitPhase::Closed
    );
}

#[test]
fn leaving_open_takes_two_consecutive_closed_reads() {
    let now = Instant::now();
    let closing = MenuWaitPhase::Open.observe(false, now);
    assert_eq!(closing, MenuWaitPhase::Closing);
    assert_eq!(closing.observe(true, now), MenuWaitPhase::Open);
    assert_eq!(closing.observe(false, now), MenuWaitPhase::Closed);
}

#[test]
fn the_deadline_closes_only_a_grace_whose_reads_were_all_closed() {
    let until = Instant::now() + Duration::from_secs(1);
    assert_eq!(awaiting(until, true).expire(), MenuWaitPhase::Closed);
    assert_eq!(awaiting(until, false).expire(), awaiting(until, false));
    assert_eq!(MenuWaitPhase::Open.expire(), MenuWaitPhase::Open);
    assert_eq!(MenuWaitPhase::Closing.expire(), MenuWaitPhase::Closing);
}
