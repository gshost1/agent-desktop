use super::*;
use agent_desktop_core::StepMechanism;
use std::collections::VecDeque;

struct ScriptedRoutes {
    semantic: Option<&'static str>,
    effects: VecDeque<bool>,
    physical_error: Option<AdapterError>,
    physical_calls: usize,
}

impl ScriptedRoutes {
    fn new(semantic: Option<&'static str>, effects: &[bool]) -> Self {
        Self {
            semantic,
            effects: effects.iter().copied().collect(),
            physical_error: None,
            physical_calls: 0,
        }
    }
}

impl ModalRoutes for ScriptedRoutes {
    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError> {
        Ok(self
            .semantic
            .map(|label| ActionStep::succeeded(label).with_mechanism(StepMechanism::SemanticApi)))
    }

    fn physical(&mut self) -> Result<ActionStep, AdapterError> {
        self.physical_calls += 1;
        match self.physical_error.take() {
            Some(error) => Err(error),
            None => Ok(ActionStep::succeeded("CGEventPostToPid")
                .with_mechanism(StepMechanism::PhysicalSynthetic)),
        }
    }

    fn effect_observed(&mut self) -> Result<bool, AdapterError> {
        Ok(self.effects.pop_front().unwrap_or(false))
    }
}

fn labels(steps: &[ActionStep]) -> Vec<&str> {
    steps.iter().map(ActionStep::label).collect()
}

#[test]
fn a_semantic_confirm_the_sheet_ignored_is_followed_by_a_real_key() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[false, true]);
    let (observed, steps) = run_routes(&mut routes).unwrap();
    assert!(observed);
    assert_eq!(routes.physical_calls, 1);
    assert_eq!(labels(&steps), ["AXConfirm", "CGEventPostToPid"]);
}

#[test]
fn a_semantic_success_alone_is_never_reported_as_an_effect() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[false, false]);
    let (observed, steps) = run_routes(&mut routes).unwrap();
    assert!(!observed);
    assert_eq!(labels(&steps), ["AXConfirm", "CGEventPostToPid"]);
}

#[test]
fn a_semantic_confirm_that_closed_the_sheet_sends_no_second_key() {
    let mut routes = ScriptedRoutes::new(Some("AXCancel"), &[true]);
    let (observed, steps) = run_routes(&mut routes).unwrap();
    assert!(observed);
    assert_eq!(routes.physical_calls, 0);
    assert_eq!(labels(&steps), ["AXCancel"]);
}

#[test]
fn a_control_without_the_semantic_action_gets_the_real_key_only() {
    let mut routes = ScriptedRoutes::new(None, &[true]);
    let (observed, steps) = run_routes(&mut routes).unwrap();
    assert!(observed);
    assert_eq!(labels(&steps), ["CGEventPostToPid"]);
}

#[test]
fn a_real_key_refused_after_a_semantic_confirm_keeps_the_delivery_uncertain() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[false]);
    routes.physical_error = Some(AdapterError::new(ErrorCode::ActionFailed, "no focus"));
    let error = run_routes(&mut routes).unwrap_err();
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_real_key_refused_before_any_delivery_stays_undelivered() {
    let mut routes = ScriptedRoutes::new(None, &[]);
    routes.physical_error = Some(AdapterError::new(ErrorCode::ActionFailed, "no focus"));
    let error = run_routes(&mut routes).unwrap_err();
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn sheets_and_dialogs_are_modal_and_standard_windows_are_not() {
    assert_eq!(modal_kind(Some("AXSheet"), None), Some("sheet"));
    assert_eq!(modal_kind(Some("AXWindow"), Some("AXSheet")), Some("sheet"));
    assert_eq!(
        modal_kind(Some("AXWindow"), Some("AXDialog")),
        Some("dialog")
    );
    assert_eq!(
        modal_kind(Some("AXWindow"), Some("AXSystemDialog")),
        Some("dialog")
    );
    assert_eq!(modal_kind(Some("AXWindow"), Some("AXStandardWindow")), None);
    assert_eq!(modal_kind(Some("AXWindow"), Some("AXFloatingWindow")), None);
    assert_eq!(modal_kind(None, None), None);
}

#[test]
fn return_reads_the_default_button_and_escape_the_cancel_button() {
    assert_eq!(
        button_attribute("AXConfirm"),
        ("AXDefaultButton", "default_button")
    );
    assert_eq!(
        button_attribute("AXCancel"),
        ("AXCancelButton", "cancel_button")
    );
}

#[test]
fn only_an_invalid_or_roleless_surface_counts_as_closed() {
    assert!(surface_read_shows_closed(Err(
        accessibility_sys::kAXErrorInvalidUIElement
    )));
    assert!(surface_read_shows_closed(Ok(None)));
    assert!(!surface_read_shows_closed(Ok(Some("AXSheet".into()))));
    assert!(!surface_read_shows_closed(Err(
        accessibility_sys::kAXErrorCannotComplete
    )));
}

#[test]
fn an_ignored_confirm_fails_unsafe_to_retry_and_names_the_ref_route() {
    let watch = ModalWatch {
        surface: crate::tree::element_for_pid(std::process::id() as i32),
        kind: "sheet",
        action: "AXConfirm",
        pid: std::process::id() as i32,
        button: ("default_button", false),
        windows: Vec::new(),
    };
    let steps = [
        ActionStep::succeeded("AXConfirm"),
        ActionStep::succeeded("CGEventPostToPid"),
    ];
    let error = watch.ignored(&steps);
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("modal details");
    assert_eq!(details["modal_key"]["effect"], "none_observed");
    assert_eq!(details["modal_key"]["surface"], "sheet");
    assert_eq!(details["modal_key"]["default_button"], false);
    assert_eq!(
        details["modal_key"]["routes"],
        serde_json::json!(["AXConfirm", "CGEventPostToPid"])
    );
    let suggestion = error.suggestion.expect("suggestion");
    assert!(suggestion.contains("Do not repeat the key"), "{suggestion}");
    assert!(suggestion.contains("--headed"), "{suggestion}");
    assert!(suggestion.contains("--surface sheet"), "{suggestion}");
}

#[test]
fn an_ignored_dialog_with_a_default_button_points_at_that_button() {
    let hint = ignored_suggestion("dialog", ("default_button", true));
    assert!(hint.contains("--surface alert"), "{hint}");
    assert!(hint.contains("default button by ref"), "{hint}");
    let hint = ignored_suggestion("sheet", ("cancel_button", true));
    assert!(hint.contains("cancel button by ref"), "{hint}");
}

#[test]
fn an_observed_effect_is_a_verified_delivery_naming_the_route() {
    let watch = ModalWatch {
        surface: crate::tree::element_for_pid(std::process::id() as i32),
        kind: "sheet",
        action: "AXConfirm",
        pid: std::process::id() as i32,
        button: ("default_button", false),
        windows: Vec::new(),
    };
    let result = watch.verified(vec![
        ActionStep::succeeded("AXConfirm"),
        ActionStep::succeeded("CGEventPostToPid").with_verified(true),
    ]);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["disposition"]["delivery"], "delivered_verified");
    assert_eq!(value["details"]["modal_key"]["effect"], "observed");
    assert_eq!(
        value["details"]["modal_key"]["observed_after"],
        "CGEventPostToPid"
    );
}
