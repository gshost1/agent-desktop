use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{Action, Direction, ElementState};

struct ProtocolAdapter {
    delivers: bool,
}

impl ObservationOps for ProtocolAdapter {
    fn delivers_action_without_pointer(&self, _handle: &NativeHandle, _action: &Action) -> bool {
        self.delivers
    }
}
impl ActionOps for ProtocolAdapter {}
impl InputOps for ProtocolAdapter {}
impl SystemOps for ProtocolAdapter {}

fn evidence(nonspatial: bool) -> ActionabilityEvidence {
    ActionabilityEvidence {
        nonspatial,
        state: ElementState {
            role: "button".into(),
            states: Vec::new(),
            value: None,
            enabled: Some(true),
            hidden: Some(false),
            offscreen: Some(false),
        },
        states_complete: true,
        bounds: None,
        available_actions: vec![crate::actions::capability::CLICK.into()],
    }
}

fn report(
    adapter: &ProtocolAdapter,
    nonspatial: bool,
    request: &ActionRequest,
) -> Result<super::super::report::ActionabilityReport, crate::AdapterError> {
    let handle = NativeHandle::null();
    let deadline = crate::Deadline::after(5_000).expect("deadline");
    super::super::evaluate::build_report(
        super::super::StabilityExpectation::permissive(None),
        &evidence(nonspatial),
        request,
        Some((&handle, adapter)),
        deadline,
    )
}

#[test]
fn a_protocol_target_takes_a_headless_double_click_and_scroll_without_bounds() {
    let adapter = ProtocolAdapter { delivers: true };
    for action in [Action::DoubleClick, Action::Scroll(Direction::Down, 3)] {
        let report =
            report(&adapter, true, &ActionRequest::headless(action.clone())).expect("report");
        assert!(
            report.actionable,
            "{action:?}: {}",
            report.failure_reasons()
        );
    }
}

#[test]
fn without_the_adapter_declaring_delivery_a_headless_double_click_stays_denied() {
    let adapter = ProtocolAdapter { delivers: false };
    let outcome = report(
        &adapter,
        true,
        &ActionRequest::headless(Action::DoubleClick),
    );
    assert!(outcome.map_or(true, |report| !report.actionable));
}

#[test]
fn a_target_with_a_screen_position_never_skips_the_pointer_gates() {
    let adapter = ProtocolAdapter { delivers: true };
    let outcome = report(
        &adapter,
        false,
        &ActionRequest::headless(Action::DoubleClick),
    );
    assert!(outcome.map_or(true, |report| !report.actionable));
}

#[test]
fn a_headed_request_never_uses_protocol_delivery() {
    let adapter = ProtocolAdapter { delivers: true };
    let handle = NativeHandle::null();
    assert!(!applies(
        &evidence(true),
        &ActionRequest::headed(Action::DoubleClick),
        Some((&handle, &adapter)),
    ));
}
