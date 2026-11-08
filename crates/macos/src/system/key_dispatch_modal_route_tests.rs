use super::*;
use agent_desktop_core::ErrorCode;
use std::collections::VecDeque;

struct ScriptedRoutes {
    semantic: Option<&'static str>,
    focus_moved: bool,
    effects: VecDeque<Option<&'static str>>,
    physical_error: Option<AdapterError>,
    calls: Vec<&'static str>,
}

impl ScriptedRoutes {
    fn new(semantic: Option<&'static str>, effects: &[Option<&'static str>]) -> Self {
        Self {
            semantic,
            focus_moved: false,
            effects: effects.iter().copied().collect(),
            physical_error: None,
            calls: Vec::new(),
        }
    }
}

impl ModalRoutes for ScriptedRoutes {
    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError> {
        self.calls.push("semantic");
        Ok(self
            .semantic
            .map(|label| ActionStep::succeeded(label).with_mechanism(StepMechanism::SemanticApi)))
    }

    fn restore_focus(&mut self) -> Result<Option<ActionStep>, AdapterError> {
        self.calls.push("restore_focus");
        Ok(self
            .focus_moved
            .then(|| ActionStep::succeeded("AXFocused").with_verified(true)))
    }

    fn physical(&mut self) -> Result<ActionStep, AdapterError> {
        self.calls.push("physical");
        match self.physical_error.take() {
            Some(error) => Err(error),
            None => Ok(ActionStep::succeeded("CGEventPostToPid")
                .with_mechanism(StepMechanism::PhysicalSynthetic)),
        }
    }

    fn effect(&mut self) -> Result<Option<&'static str>, AdapterError> {
        self.calls.push("effect");
        Ok(self.effects.pop_front().flatten())
    }
}

fn labels(steps: &[ActionStep]) -> Vec<&str> {
    steps.iter().map(ActionStep::label).collect()
}

#[test]
fn a_semantic_confirm_the_sheet_ignored_is_followed_by_a_real_key() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[None, Some("surface_closed")]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, Some("surface_closed"));
    assert_eq!(labels(&steps), ["AXConfirm", "CGEventPostToPid"]);
}

#[test]
fn a_semantic_success_alone_is_never_reported_as_an_effect() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[None, None]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, None);
    assert_eq!(labels(&steps), ["AXConfirm", "CGEventPostToPid"]);
}

#[test]
fn focus_moved_by_an_ignored_confirm_goes_back_before_the_real_key() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[None, Some("surface_closed")]);
    routes.focus_moved = true;
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, Some("surface_closed"));
    assert_eq!(
        labels(&steps),
        ["AXConfirm", "AXFocused", "CGEventPostToPid"]
    );
    assert_eq!(
        routes.calls,
        ["semantic", "effect", "restore_focus", "physical", "effect"]
    );
}

#[test]
fn a_semantic_confirm_that_closed_the_sheet_sends_no_second_key() {
    let mut routes = ScriptedRoutes::new(Some("AXCancel"), &[Some("surface_closed")]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, Some("surface_closed"));
    assert_eq!(routes.calls, ["semantic", "effect"]);
    assert_eq!(labels(&steps), ["AXCancel"]);
}

#[test]
fn a_control_without_the_semantic_action_gets_the_real_key_only() {
    let mut routes = ScriptedRoutes::new(None, &[Some("surface_replaced")]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, Some("surface_replaced"));
    assert_eq!(routes.calls, ["semantic", "physical", "effect"]);
    assert_eq!(labels(&steps), ["CGEventPostToPid"]);
}

#[test]
fn a_real_key_refused_after_a_semantic_confirm_keeps_the_delivery_uncertain() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[None]);
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
