use super::{CommandFocus, retry_leased_point_phase};
use crate::{AdapterError, AppError, ErrorCode};
use serde_json::json;

const TIMEOUT_MS: u64 = 400;

fn adapter_error(error: AppError) -> AdapterError {
    match error {
        AppError::Adapter(error) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

fn refusal(check: &str) -> AppError {
    AdapterError::new(ErrorCode::ActionFailed, "not actionable")
        .with_details(json!({ "check": check }))
        .into()
}

fn occluded() -> AppError {
    AdapterError::new(ErrorCode::ActionFailed, "Target is not actionable: receives_events (occluded by button)")
        .with_details(json!({ "checks": [
            { "check": "visible", "status": "pass" },
            { "check": "receives_events", "status": "fail", "reason": "occluded by button",
              "occluder": { "role": "button", "bounds": { "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 } } },
        ] }))
        .into()
}

fn leased_wait<T>(
    timeout_ms: u64,
    mut attempt: impl FnMut(u32) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let deadline = crate::Deadline::after(timeout_ms).unwrap();
    let mut attempts = 0;
    retry_leased_point_phase(
        Some(timeout_ms),
        deadline,
        &mut CommandFocus::default(),
        |_| {
            attempts += 1;
            attempt(attempts)
        },
    )
}

#[test]
fn a_deadline_inside_an_attempt_keeps_the_occlusion_that_sent_the_wait_back() {
    let error = leased_wait::<()>(TIMEOUT_MS, |attempt| {
        if attempt == 1 {
            return Err(occluded());
        }
        Err(AdapterError::timeout("Operation exceeded its deadline").into())
    })
    .unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    let error = adapter_error(error);
    let refusal = &error.details.as_ref().unwrap()["refusal"];
    assert_eq!(refusal["code"], "ACTION_FAILED");
    assert_eq!(
        refusal["details"]["checks"][1]["occluder"]["role"],
        "button"
    );
    assert!(
        error
            .suggestion
            .as_deref()
            .unwrap_or("")
            .contains("covered the target")
    );
}

#[test]
fn the_waits_own_timeout_keeps_the_last_refusal_and_names_its_check() {
    let error = leased_wait::<()>(150, |_| Err(refusal("stable"))).unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    let error = adapter_error(error);
    assert_eq!(
        error.details.as_ref().unwrap()["refusal"]["details"]["check"],
        "stable"
    );
    assert!(
        error
            .suggestion
            .as_deref()
            .unwrap_or("")
            .contains("still moving")
    );
}

#[test]
fn an_error_that_is_not_a_timeout_carries_no_earlier_refusal() {
    let error = leased_wait::<()>(TIMEOUT_MS, |attempt| {
        if attempt == 1 {
            return Err(occluded());
        }
        Err(AdapterError::new(ErrorCode::ActionNotSupported, "no pointer").into())
    })
    .unwrap_err();

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
    let error = adapter_error(error);
    assert!(
        error
            .details
            .as_ref()
            .is_none_or(|details| details.get("refusal").is_none())
    );
}
