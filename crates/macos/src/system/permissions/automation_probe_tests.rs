use super::*;
use std::time::Instant;

fn granted() -> i32 {
    0
}

fn stuck_behind_a_prompt() -> i32 {
    std::thread::sleep(Duration::from_secs(5));
    0
}

#[test]
fn an_answering_probe_returns_its_status() {
    assert_eq!(probe_within(Deadline::after(1_000).unwrap(), granted), 0);
}

#[test]
fn a_probe_blocked_behind_a_prompt_gives_up_at_the_deadline() {
    let started = Instant::now();

    let status = probe_within(Deadline::after(100).unwrap(), stuck_behind_a_prompt);

    assert_eq!(status, PROBE_UNANSWERED);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_long_command_deadline_still_bounds_the_probe() {
    let started = Instant::now();

    let status = probe_within(Deadline::after(60_000).unwrap(), stuck_behind_a_prompt);

    assert_eq!(status, PROBE_UNANSWERED);
    assert!(started.elapsed() < AUTOMATION_PROBE_BUDGET + Duration::from_millis(500));
}

#[test]
fn an_unanswered_probe_is_an_unknown_permission() {
    assert_eq!(
        super::super::map_automation_probe(PROBE_UNANSWERED),
        agent_desktop_core::PermissionState::Unknown
    );
}
