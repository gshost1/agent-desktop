use super::*;
use agent_desktop_core::{ErrorCode, StepMechanism};
use std::collections::VecDeque;

struct ScriptedRoutes {
    semantic: Option<&'static str>,
    effects: VecDeque<Option<&'static str>>,
    physical_error: Option<AdapterError>,
    calls: Vec<&'static str>,
}

impl ScriptedRoutes {
    fn new(semantic: Option<&'static str>, effects: &[Option<&'static str>]) -> Self {
        Self {
            semantic,
            effects: effects.iter().copied().collect(),
            physical_error: None,
            calls: Vec::new(),
        }
    }
}

impl ModalRoutes for ScriptedRoutes {
    fn physical(&mut self) -> Result<ActionStep, AdapterError> {
        self.calls.push("physical");
        match self.physical_error.take() {
            Some(error) => Err(error),
            None => Ok(ActionStep::succeeded("CGEventPostToPid")
                .with_mechanism(StepMechanism::PhysicalSynthetic)),
        }
    }

    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError> {
        self.calls.push("semantic");
        Ok(self
            .semantic
            .map(|label| ActionStep::succeeded(label).with_mechanism(StepMechanism::SemanticApi)))
    }

    fn effect(&mut self) -> Result<Option<&'static str>, AdapterError> {
        self.calls.push("effect");
        Ok(self.effects.pop_front().flatten())
    }
}

fn labels(steps: &[ActionStep]) -> Vec<&str> {
    steps.iter().map(ActionStep::label).collect()
}

fn refused() -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "no focused element")
        .with_disposition(DeliverySemantics::not_delivered())
}

#[test]
fn the_real_key_goes_first_and_no_semantic_action_follows_a_reaction() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[Some("surface_window_closed")]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, Some("surface_window_closed"));
    assert_eq!(routes.calls, ["physical", "effect"]);
    assert_eq!(labels(&steps), ["CGEventPostToPid"]);
}

#[test]
fn a_real_key_the_sheet_ignored_is_followed_by_the_semantic_action() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[None, Some("surface_window_closed")]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, Some("surface_window_closed"));
    assert_eq!(routes.calls, ["physical", "effect", "semantic", "effect"]);
    assert_eq!(labels(&steps), ["CGEventPostToPid", "AXConfirm"]);
}

#[test]
fn a_semantic_success_alone_is_never_reported_as_an_effect() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[None, None]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, None);
    assert_eq!(labels(&steps), ["CGEventPostToPid", "AXConfirm"]);
}

#[test]
fn an_ignored_real_key_without_a_semantic_action_reports_no_effect() {
    let mut routes = ScriptedRoutes::new(None, &[None]);
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, None);
    assert_eq!(labels(&steps), ["CGEventPostToPid"]);
}

#[test]
fn a_refused_real_key_falls_back_to_the_semantic_action() {
    let mut routes = ScriptedRoutes::new(Some("AXCancel"), &[Some("surface_window_closed")]);
    routes.physical_error = Some(refused());
    let (signal, steps) = run_routes(&mut routes).unwrap();
    assert_eq!(signal, Some("surface_window_closed"));
    assert_eq!(routes.calls, ["physical", "semantic", "effect"]);
    assert_eq!(labels(&steps), ["AXCancel"]);
}

#[test]
fn a_refused_real_key_with_no_semantic_action_returns_the_refusal_undelivered() {
    let mut routes = ScriptedRoutes::new(None, &[]);
    routes.physical_error = Some(refused());
    let error = run_routes(&mut routes).unwrap_err();
    assert_eq!(error.message, "no focused element");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_partly_delivered_real_key_is_not_followed_by_another_route() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[]);
    routes.physical_error = Some(
        AdapterError::new(ErrorCode::ActionFailed, "key up lost")
            .with_disposition(DeliverySemantics::delivered_unverified()),
    );
    let error = run_routes(&mut routes).unwrap_err();
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(routes.calls, ["physical"]);
}

#[test]
fn an_unobservable_surface_gets_the_real_key_and_nothing_after_it() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[]);
    let step = first_delivered_step(&mut routes).unwrap();
    assert_eq!(step.label(), "CGEventPostToPid");
    assert_eq!(routes.calls, ["physical"]);
}

#[test]
fn an_unobservable_surface_falls_back_only_when_the_key_was_never_sent() {
    let mut routes = ScriptedRoutes::new(Some("AXConfirm"), &[]);
    routes.physical_error = Some(refused());
    let step = first_delivered_step(&mut routes).unwrap();
    assert_eq!(step.label(), "AXConfirm");
    assert_eq!(routes.calls, ["physical", "semantic"]);
}
