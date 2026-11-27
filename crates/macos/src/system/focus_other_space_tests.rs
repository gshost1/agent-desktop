use super::*;
use agent_desktop_core::ErrorCode;

fn away(kind: &str) -> AdapterError {
    AdapterError::new(ErrorCode::ActionNotSupported, "away").with_details(serde_json::json!({
        "kind": kind,
        "window_id": "w-9",
    }))
}

fn deadline() -> Deadline {
    Deadline::standard().unwrap()
}

#[test]
fn the_element_is_returned_once_the_space_switch_publishes_it() {
    let mut calls = 0;
    let element = element_after_activation(
        || {
            calls += 1;
            match calls {
                1 => Err(away("window_on_other_space")),
                2 => Err(away("window_without_accessibility_element")),
                _ => Ok("element"),
            }
        },
        true,
        deadline(),
    )
    .unwrap();
    assert_eq!(element, "element");
    assert_eq!(calls, 3);
}

#[test]
fn another_error_ends_the_wait_at_once() {
    let mut calls = 0;
    let error = element_after_activation::<()>(
        || {
            calls += 1;
            Err(AdapterError::new(ErrorCode::WindowNotFound, "closed"))
        },
        true,
        deadline(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(calls, 1);
}

#[test]
fn a_window_that_never_arrives_names_mission_control_not_focus_window() {
    let started = Instant::now();
    let error =
        element_after_activation::<()>(|| Err(away("window_on_other_space")), true, deadline())
            .unwrap_err();
    assert!(started.elapsed() >= SPACE_SWITCH_LIMIT);
    assert!(started.elapsed() < SPACE_SWITCH_LIMIT + Duration::from_secs(1));
    assert!(is_other_space_error(&error));
    assert_eq!(
        error.details.as_ref().unwrap()["application_activated"],
        true
    );
    assert!(error.message.contains("w-9"));
    let suggestion = error.suggestion.as_deref().unwrap();
    assert!(suggestion.contains("Mission Control"));
    assert!(!suggestion.contains("focus-window"));
}

#[test]
fn a_window_on_screen_without_its_element_keeps_that_error() {
    let error = element_after_activation::<()>(
        || Err(away("window_without_accessibility_element")),
        true,
        deadline(),
    )
    .unwrap_err();
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["kind"], "window_without_accessibility_element");
    assert_eq!(details["window_id"], "w-9");
    assert_eq!(details["application_activated"], true);
    assert!(!error.message.contains("stayed on another Space"));
}

#[test]
fn the_command_deadline_running_out_is_a_timeout() {
    let short = Deadline::after(60).unwrap();
    let error = element_after_activation::<()>(|| Err(away("window_on_other_space")), true, short)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["kind"], "window_space_switch");
    assert_eq!(details["window_id"], "w-9");
    assert_eq!(details["application_activated"], true);
}

#[test]
fn the_suggestion_covers_an_application_with_a_window_on_screen() {
    let error =
        element_after_activation::<()>(|| Err(away("window_on_other_space")), true, deadline())
            .unwrap_err();
    let suggestion = error.suggestion.as_deref().unwrap();
    assert!(suggestion.contains("did not switch Spaces"));
    assert!(suggestion.contains("no window on the Space on screen"));
}

#[test]
fn a_lookup_reporting_the_deadline_becomes_the_space_switch_timeout() {
    let short = Deadline::after(60).unwrap();
    let mut calls = 0;
    let error = element_after_activation::<()>(
        || {
            calls += 1;
            if short.is_expired() {
                Err(short.timeout_error())
            } else {
                Err(away("window_on_other_space"))
            }
        },
        true,
        short,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["kind"], "window_space_switch");
    assert_eq!(details["window_id"], "w-9");
    assert!(error.message.contains("did not come on screen"));
    assert!(error.suggestion.as_deref().unwrap().contains("--timeout"));
    assert!(calls >= 1);
}

#[test]
fn a_window_once_seen_on_another_space_stayed_there() {
    let mut calls = 0;
    let error = element_after_activation::<()>(
        || {
            calls += 1;
            Err(away(if calls == 1 {
                "window_on_other_space"
            } else {
                "window_without_accessibility_element"
            }))
        },
        true,
        deadline(),
    )
    .unwrap_err();
    assert!(is_other_space_error(&error));
    assert!(error.message.contains("stayed on another Space"));
}

#[test]
fn an_activation_that_did_not_start_is_not_reported_as_activated() {
    let error =
        element_after_activation::<()>(|| Err(away("window_on_other_space")), false, deadline())
            .unwrap_err();
    assert_eq!(
        error.details.as_ref().unwrap()["application_activated"],
        false
    );
}
