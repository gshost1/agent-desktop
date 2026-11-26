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
        deadline(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(calls, 1);
}

#[test]
fn a_window_that_never_arrives_names_mission_control_not_focus_window() {
    let started = Instant::now();
    let error = element_after_activation::<()>(|| Err(away("window_on_other_space")), deadline())
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
