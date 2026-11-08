use super::{PressArgs, execute};
use crate::AdapterError;
use crate::actions::action::Action;
use crate::actions::action_request::ActionRequest;
use crate::actions::action_result::ActionResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::helpers::test_support::save_one_ref_snapshot;
use crate::context::CommandContext;
use crate::refs::RefEntry;
use crate::refs::refs_test_support::HomeGuard;
use std::sync::Mutex;

#[derive(Default)]
struct KeyTargetAdapter {
    pressed: Mutex<Vec<Action>>,
    global_presses: Mutex<usize>,
}

impl ObservationOps for KeyTargetAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new("field"))
    }

    crate::adapter::complete_live_observation!(
        "textfield",
        "Target",
        [crate::actions::capability::SET_FOCUS]
    );
    crate::adapter::saved_ref_scope!(nodes: &[("textfield", Some("Target"), None)]);
}

impl ActionOps for KeyTargetAdapter {
    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        if handle.is_null() {
            *self.global_presses.lock().unwrap() += 1;
        } else {
            self.pressed.lock().unwrap().push(request.action);
        }
        Ok(ActionResult::delivered_unverified("PressKey"))
    }
}

impl InputOps for KeyTargetAdapter {}

impl SystemOps for KeyTargetAdapter {
    crate::adapter::guarded_interaction_lease!();
}

fn press_into(ref_id: String) -> PressArgs {
    PressArgs {
        combo: "return".into(),
        app: None,
        window_id: None,
        ref_id: Some(ref_id),
        force: false,
    }
}

#[test]
fn a_ref_receives_the_key_as_an_element_action() {
    let _home = HomeGuard::new();
    let snapshot_id = save_one_ref_snapshot("textfield", "SetFocus");
    let adapter = KeyTargetAdapter::default();

    execute(
        press_into(format!("@{snapshot_id}:e1")),
        &adapter,
        &CommandContext::default(),
    )
    .expect("a key pressed into a live ref is delivered to that element");

    let pressed = adapter.pressed.lock().unwrap();
    assert!(
        matches!(pressed.as_slice(), [Action::PressKey(combo)] if combo.key == "return" && combo.modifiers.is_empty()),
        "got {pressed:?}"
    );
    assert_eq!(*adapter.global_presses.lock().unwrap(), 0);
}

#[test]
fn a_ref_with_an_app_target_is_refused_before_any_key_is_sent() {
    let _home = HomeGuard::new();
    let snapshot_id = save_one_ref_snapshot("textfield", "SetFocus");
    let adapter = KeyTargetAdapter::default();
    let mut args = press_into(format!("@{snapshot_id}:e1"));
    args.app = Some("Browser".into());

    let error = execute(args, &adapter, &CommandContext::default()).unwrap_err();

    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(adapter.pressed.lock().unwrap().is_empty());
    assert_eq!(*adapter.global_presses.lock().unwrap(), 0);
}
