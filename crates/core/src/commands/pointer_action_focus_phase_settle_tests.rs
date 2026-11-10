use super::{AttemptFocus, retry_leased_point_phase};
use crate::{AdapterError, AppError, ErrorCode};
use serde_json::json;

const TIMEOUT_MS: u64 = 1_000;

fn refusal(check: &str) -> AppError {
    AdapterError::new(ErrorCode::ActionFailed, "settling")
        .with_details(json!({ "check": check }))
        .into()
}

fn moved_focus(error: AppError) -> bool {
    match error {
        AppError::Adapter(error) => error.moved_focus(),
        _ => false,
    }
}

#[test]
fn a_raise_that_leaves_the_target_settling_goes_back_to_the_wait() {
    for check in ["visible", "stable"] {
        let deadline = crate::Deadline::after(TIMEOUT_MS).unwrap();
        let mut attempts = 0;

        let value = retry_leased_point_phase(Some(TIMEOUT_MS), deadline, |focus| {
            attempts += 1;
            if attempts == 1 {
                *focus = AttemptFocus::FrontChanged;
                return Err(refusal(check));
            }
            *focus = AttemptFocus::AlreadyFront;
            Ok(attempts)
        })
        .unwrap();

        assert_eq!(
            value, 2,
            "a raise followed by a failed {check} check stopped"
        );
    }
}

#[test]
fn a_raise_that_leaves_the_target_covered_ends_the_command_even_while_it_settles() {
    let deadline = crate::Deadline::after(TIMEOUT_MS).unwrap();
    let mut attempts = 0;

    let error = retry_leased_point_phase::<()>(Some(TIMEOUT_MS), deadline, |focus| {
        attempts += 1;
        *focus = AttemptFocus::FrontChanged;
        Err(AdapterError::new(ErrorCode::ActionFailed, "covered")
            .with_details(json!({ "checks": [
                { "check": "stable", "status": "fail" },
                { "check": "receives_events", "status": "fail" },
            ] }))
            .into())
    })
    .unwrap_err();

    assert_eq!(attempts, 1);
    assert_eq!(error.code(), "ACTION_FAILED");
}

#[test]
fn the_timeout_after_an_attempt_whose_focus_ran_carries_preparatory_focus() {
    let deadline = crate::Deadline::after(150).unwrap();

    let error = retry_leased_point_phase::<()>(Some(150), deadline, |focus| {
        *focus = AttemptFocus::AlreadyFront;
        Err(refusal("receives_events"))
    })
    .unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    assert!(moved_focus(error));
}

#[test]
fn the_timeout_after_an_attempt_with_no_focus_call_carries_no_preparatory_focus() {
    let deadline = crate::Deadline::after(150).unwrap();

    let error =
        retry_leased_point_phase::<()>(Some(150), deadline, |_| Err(refusal("receives_events")))
            .unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    assert!(!moved_focus(error));
}
