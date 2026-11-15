use std::time::{Duration, Instant};

use super::super::{evaluate::check_with_stability, stability::StabilityExpectation};
use super::*;
use crate::{InteractionPolicy, Rect};

fn cell_evidence() -> ActionabilityEvidence {
    ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: "cell".to_string(),
            states: Vec::new(),
            value: None,
            enabled: None,
            hidden: None,
            offscreen: None,
        },
        states_complete: true,
        bounds: None,
        available_actions: Vec::new(),
    }
}

fn failed_editable_check() -> super::super::check::ActionabilityCheck {
    super::super::check::ActionabilityCheck {
        check: "editable",
        status: super::super::status::ActionabilityStatus::Fail,
        reason: None,
        occluder: None,
        terminal_code: Some(ErrorCode::ActionNotSupported),
        hit_test: None,
        stability: None,
    }
}

#[test]
fn non_editable_cell_suggestion_does_not_promise_an_editor_exists() {
    let evidence = cell_evidence();
    let report = ActionabilityReport::from_checks(
        vec![failed_editable_check()],
        None,
        None,
        None,
        super::super::pointer_delivery::PointerDelivery::NotApplicable,
    );

    let request = ActionRequest::headed(crate::Action::SetValue("value".into()));
    let error = finish(
        &evidence,
        &request,
        report,
        crate::Deadline::standard().unwrap(),
    )
    .expect_err("non-actionable report must fail");

    assert_eq!(
        error.suggestion.as_deref(),
        Some(
            "This cell is not directly editable. Activate it, take a fresh snapshot, and \
             target the text editor if one appears."
        )
    );
    assert!(!error.suggestion.unwrap().contains("it exposes"));
}

#[test]
fn unsupported_headed_select_does_not_suggest_repeating_headed_select() {
    let mut evidence = cell_evidence();
    evidence.state.role = "combobox".into();
    evidence.state.enabled = Some(true);
    evidence.state.hidden = Some(false);
    evidence.state.offscreen = Some(false);
    let error = check_with_stability(
        StabilityExpectation::permissive(None),
        &evidence,
        &ActionRequest::headed(crate::Action::Select("Manual Schedule".into())),
        None,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    let suggestion = error.suggestion.unwrap();
    assert!(!suggestion.contains("--headed"));
    assert!(suggestion.contains("available_actions"));
}

fn live_evidence(enabled: bool, available_actions: Vec<String>) -> ActionabilityEvidence {
    ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: "button".into(),
            states: Vec::new(),
            value: None,
            enabled: Some(enabled),
            hidden: Some(false),
            offscreen: Some(false),
        },
        states_complete: true,
        bounds: Some(Rect {
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 24.0,
        }),
        available_actions,
    }
}

fn started_ago(elapsed: Duration) -> crate::Deadline {
    let started = Instant::now().checked_sub(elapsed).unwrap();
    crate::Deadline::at(started, 5_000).unwrap()
}

fn refuse(
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    deadline: crate::Deadline,
) -> AdapterError {
    check_with_stability(
        StabilityExpectation::permissive(None),
        evidence,
        request,
        None,
        deadline,
    )
    .unwrap_err()
}

fn clickable() -> Vec<String> {
    vec![crate::actions::capability::CLICK.to_string()]
}

#[test]
fn a_target_still_disabled_after_the_settle_time_gets_a_final_disabled_refusal() {
    let error = refuse(
        &live_evidence(false, clickable()),
        &ActionRequest::headless(Action::Click),
        started_ago(DISABLED_SETTLE + Duration::from_millis(100)),
    );

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(!error.permits_retry_by_default());
    assert!(
        error.message.starts_with("Target is disabled"),
        "{}",
        error.message
    );
    let details = error.details.clone().unwrap();
    assert_eq!(details["kind"], "target_disabled");
    assert_eq!(details["retryable"], false);
    assert!(
        details["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["check"] == "enabled" && check["status"] == "fail")
    );
    let hint = error.suggestion.unwrap();
    assert!(hint.contains("disabled"), "{hint}");
    assert!(hint.contains("--predicate enabled"), "{hint}");
    assert!(!hint.contains("busy"), "{hint}");
}

#[test]
fn a_target_disabled_within_the_settle_time_stays_waitable_and_says_it_is_disabled() {
    let error = refuse(
        &live_evidence(false, clickable()),
        &ActionRequest::headless(Action::Click),
        started_ago(Duration::from_millis(200)),
    );

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(error.permits_retry_by_default());
    assert_eq!(
        error.message,
        "Target is not actionable: enabled (live enabled state is false)"
    );
    assert!(error.details.unwrap().get("kind").is_none());
    assert!(error.suggestion.unwrap().contains("disabled"));
}

#[test]
fn the_default_budget_passed_explicitly_still_ends_a_disabled_wait_early() {
    let request = ActionRequest::headless(Action::Click)
        .with_timeout_ms(Some(crate::DEFAULT_OPERATION_TIMEOUT_MS));
    let error = refuse(
        &live_evidence(false, clickable()),
        &request,
        started_ago(DISABLED_SETTLE * 2),
    );

    assert!(!error.permits_retry_by_default());
}

#[test]
fn an_explicit_timeout_keeps_waiting_for_a_disabled_target() {
    let request = ActionRequest::headless(Action::Click).with_timeout_ms(Some(30_000));
    let error = refuse(
        &live_evidence(false, clickable()),
        &request,
        started_ago(DISABLED_SETTLE * 2),
    );

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(error.permits_retry_by_default());
}

#[test]
fn a_disabled_target_with_another_gap_keeps_the_generic_refusal() {
    let mut evidence = live_evidence(false, clickable());
    evidence.state.offscreen = Some(true);
    let error = refuse(
        &evidence,
        &ActionRequest::headless(Action::Click),
        started_ago(DISABLED_SETTLE * 2),
    );

    assert!(error.permits_retry_by_default());
    assert!(!error.suggestion.unwrap().contains("is disabled"));
}

#[test]
fn a_headless_click_without_a_semantic_action_names_headed_as_the_way_out() {
    for action in [
        Action::Click,
        Action::RightClick,
        Action::DoubleClick,
        Action::TripleClick,
    ] {
        let error = refuse(
            &live_evidence(true, Vec::new()),
            &ActionRequest::headless(action.clone()),
            started_ago(Duration::ZERO),
        );

        assert_eq!(error.code, ErrorCode::PolicyDenied, "{action:?}");
        let hint = error.suggestion.unwrap();
        assert!(
            hint.contains("rerun the command with --headed"),
            "{action:?}: {hint}"
        );
        assert!(
            hint.contains("If pointer input is acceptable"),
            "{action:?}: {hint}"
        );
        assert!(
            hint.contains("no headed pointer input"),
            "{action:?}: {hint}"
        );
        assert!(
            !hint.contains("Waiting will not help"),
            "{action:?}: {hint}"
        );
    }
}

#[test]
fn headless_typing_without_a_semantic_action_names_headed_focus_as_the_way_out() {
    let mut evidence = live_evidence(true, Vec::new());
    evidence.state.role = "textfield".into();
    let error = refuse(
        &evidence,
        &ActionRequest::headless(Action::TypeText("hello".into())),
        started_ago(Duration::ZERO),
    );

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    let hint = error.suggestion.unwrap();
    assert!(hint.contains("--headed"), "{hint}");
    assert!(hint.contains("keyboard focus"), "{hint}");
}

#[test]
fn a_disabled_target_without_a_semantic_click_says_disabled_not_headed() {
    let error = refuse(
        &live_evidence(false, Vec::new()),
        &ActionRequest::headless(Action::Click),
        started_ago(Duration::ZERO),
    );

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(!error.suggestion.unwrap().contains("--headed"));
}

#[test]
fn a_missing_action_that_headed_cannot_supply_keeps_the_generic_refusal() {
    let mut request = ActionRequest::headless(Action::Expand);
    request.policy = InteractionPolicy::headless();
    let error = refuse(
        &live_evidence(true, Vec::new()),
        &request,
        started_ago(Duration::ZERO),
    );

    assert!(!error.suggestion.unwrap().contains("--headed"));
}
