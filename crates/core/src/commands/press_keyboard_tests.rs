use super::{PressArgs, execute};
use crate::actions::action_request::ActionRequest;
use crate::actions::action_result::ActionResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::context::CommandContext;
use crate::{AdapterError, KeyCombo};
use std::sync::Mutex;

#[derive(Default)]
struct RecordingAdapter {
    calls: Mutex<Vec<&'static str>>,
    preparation_fails: bool,
}

impl RecordingAdapter {
    fn record(&self, call: &'static str) {
        self.calls.lock().unwrap().push(call);
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl ObservationOps for RecordingAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        self.record("inventory");
        Ok(vec![crate::AppInfo {
            name: "Editor".into(),
            pid: crate::ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("generation-1".into()),
            presentation: None,
        }])
    }
}

impl ActionOps for RecordingAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.record("global_press");
        Ok(ActionResult::delivered_unverified("PressKey"))
    }
}

impl InputOps for RecordingAdapter {
    fn prepare_keyboard_input(&self) -> Result<(), AdapterError> {
        self.record("prepare");
        if self.preparation_fails {
            return Err(AdapterError::internal("keyboard layout unavailable"));
        }
        Ok(())
    }
}

impl SystemOps for RecordingAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn press_key_for_app(
        &self,
        _process: crate::ProcessIdentity,
        _combo: &KeyCombo,
        _policy: crate::InteractionPolicy,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.record("app_press");
        Ok(ActionResult::delivered_unverified("PressKey"))
    }
}

fn press(
    app: Option<&str>,
    adapter: &RecordingAdapter,
) -> Result<serde_json::Value, crate::AppError> {
    let args = PressArgs {
        combo: "escape".into(),
        app: app.map(str::to_owned),
        window_id: None,
        ref_id: None,
        force: false,
    };
    execute(args, adapter, &CommandContext::default())
}

#[test]
fn keyboard_input_is_prepared_once_the_app_is_confirmed_and_before_the_key_is_sent() {
    let adapter = RecordingAdapter::default();

    press(Some("Editor"), &adapter).unwrap();

    assert_eq!(
        adapter.calls(),
        vec!["inventory", "inventory", "prepare", "app_press"]
    );
}

#[test]
fn a_failed_keyboard_preparation_still_sends_the_key() {
    let adapter = RecordingAdapter {
        preparation_fails: true,
        ..Default::default()
    };

    press(Some("Editor"), &adapter).unwrap();

    assert_eq!(adapter.calls().last(), Some(&"app_press"));
}

#[test]
fn a_press_with_no_target_app_prepares_nothing() {
    let adapter = RecordingAdapter::default();

    press(None, &adapter).unwrap();

    assert_eq!(adapter.calls(), vec!["global_press"]);
}
