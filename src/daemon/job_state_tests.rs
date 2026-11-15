use super::JobState;

#[test]
fn client_leaving_a_queued_command_abandons_it_without_stopping_the_daemon() {
    let state = JobState::default();

    assert!(!state.client_left());
    assert!(!state.start());
}

#[test]
fn client_leaving_a_running_command_stops_the_daemon() {
    let state = JobState::default();
    assert!(state.start());

    assert!(state.client_left());
}

#[test]
fn client_leaving_after_the_command_finished_is_normal() {
    let state = JobState::default();
    assert!(state.start());
    state.finish();

    assert!(!state.client_left());
}

#[test]
fn heartbeats_run_only_while_the_command_runs() {
    let state = JobState::default();
    assert!(!state.is_running());
    assert!(!state.is_settled());
    assert!(state.start());
    assert!(state.is_running());
    state.finish();

    assert!(!state.is_running());
    assert!(state.is_settled());
}

#[test]
fn a_command_its_client_abandoned_is_settled_without_running() {
    let state = JobState::default();
    assert!(!state.client_left());

    assert!(state.is_settled());
    assert!(!state.is_running());
}
