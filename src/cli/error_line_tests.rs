use super::*;
use agent_desktop_core::{AppError, ErrorCode, response::output::ErrorPayload};
use serde_json::json;

fn stale_ref_error() -> AppError {
    AppError::Adapter(
        agent_desktop_core::AdapterError::new(ErrorCode::StaleRef, "Ref no longer matches")
            .with_suggestion("Take a new snapshot"),
    )
}

#[test]
fn an_error_response_gives_one_line_with_code_message_and_suggestion() {
    let response = Response::err("click", ErrorPayload::from_app_error(&stale_ref_error()));

    assert_eq!(
        response_error_lines(&response),
        vec!["agent-desktop: STALE_REF: Ref no longer matches -- Take a new snapshot"]
    );
}

#[test]
fn a_successful_response_gives_no_line() {
    let response = Response::ok("snapshot", json!({ "ref_count": 0 }));

    assert!(response_error_lines(&response).is_empty());
}

#[test]
fn an_error_without_a_suggestion_ends_after_the_message() {
    let response = Response::err(
        "click",
        ErrorPayload::from_app_error(&AppError::invalid_input("bad ref")),
    );

    assert_eq!(
        response_error_lines(&response),
        vec!["agent-desktop: INVALID_ARGS: bad ref"]
    );
}

#[test]
fn control_characters_never_reach_the_terminal() {
    let error = AppError::invalid_input("line one\nline two\u{1b}[31m");
    let response = Response::err("click", ErrorPayload::from_app_error(&error));

    let lines = response_error_lines(&response);

    assert_eq!(lines.len(), 1);
    assert!(!lines[0].chars().any(char::is_control), "{lines:?}");
    assert!(lines[0].contains("line one line two"));
}

#[test]
fn a_long_message_is_bounded() {
    let error = AppError::invalid_input("x".repeat(5000));
    let response = Response::err("click", ErrorPayload::from_app_error(&error));

    let lines = response_error_lines(&response);

    assert!(lines[0].chars().count() < 600, "{}", lines[0].len());
}

fn batch_data() -> serde_json::Value {
    json!({ "results": [
        { "ok": true, "command": "snapshot", "index": 0, "execution": "completed", "data": {} },
        { "ok": false, "command": "click", "index": 1, "execution": "completed",
          "error": { "code": "POLICY_DENIED", "message": "denied", "suggestion": "use --headed" } },
        { "ok": false, "command": "press", "index": 2, "execution": "not_started",
          "error": { "code": "ACTION_FAILED", "message": "not run" } },
    ] })
}

#[test]
fn a_successful_batch_gives_one_line_per_failed_completed_entry() {
    let response = Response::ok("batch", batch_data());

    assert_eq!(
        response_error_lines(&response),
        vec!["agent-desktop: batch[1] click: POLICY_DENIED: denied -- use --headed"]
    );
}

#[test]
fn a_json_envelope_gives_the_same_lines_as_the_response() {
    let error = Response::err("click", ErrorPayload::from_app_error(&stale_ref_error()));
    let batch = Response::ok("batch", batch_data());
    let success = Response::ok("version", json!({}));

    for response in [error, batch, success] {
        let envelope = serde_json::to_value(&response).unwrap();
        assert_eq!(
            envelope_error_lines(&envelope),
            response_error_lines(&response)
        );
    }
}

#[test]
fn the_env_setting_turns_the_line_off_only_when_asked() {
    assert!(error_lines_enabled(None));
    assert!(error_lines_enabled(Some("1")));
    assert!(error_lines_enabled(Some("")));
    assert!(!error_lines_enabled(Some("0")));
    assert!(!error_lines_enabled(Some("off")));
    assert!(!error_lines_enabled(Some("false")));
}
