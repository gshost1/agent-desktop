use super::*;

#[test]
fn the_first_good_capture_is_read_again_without_a_pause() {
    assert!(!needs_pause(1, false));
}

#[test]
fn a_changed_or_failed_capture_is_followed_by_a_pause() {
    assert!(needs_pause(2, false));
    assert!(needs_pause(5, false));
    assert!(needs_pause(1, true));
}

#[test]
fn a_pause_never_outlasts_the_deadline() {
    let started = Instant::now();
    pause_before_next_capture(2, false, started);
    assert!(started.elapsed() < CHURN_PAUSE);
}
