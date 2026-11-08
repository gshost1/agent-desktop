use super::{PressArgs, execute};
use crate::actions::action_result::ActionResult;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::context::CommandContext;
use crate::{AdapterError, ErrorCode, KeyCombo};

struct LayoutRefusingAdapter;

impl ObservationOps for LayoutRefusingAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        Ok(vec![crate::AppInfo {
            name: "Editor".into(),
            pid: crate::ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("generation-1".into()),
            presentation: None,
        }])
    }

    fn list_windows(
        &self,
        _filter: &crate::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        Ok(vec![crate::WindowInfo {
            id: "w-42".into(),
            title: "Editor".into(),
            app: "Editor".into(),
            pid: crate::ProcessId::new(42),
            process_instance: Some("generation-1".into()),
            bounds: None,
            state: crate::WindowState {
                visible: Some(true),
                ..Default::default()
            },
        }])
    }
}

impl ActionOps for LayoutRefusingAdapter {}
impl InputOps for LayoutRefusingAdapter {}

impl SystemOps for LayoutRefusingAdapter {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();

    fn press_key_for_app(
        &self,
        _process: crate::ProcessIdentity,
        _combo: &KeyCombo,
        _policy: crate::InteractionPolicy,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "No unmodified key types 'a' in the active keyboard layout",
        ))
    }
}

fn refusal(headed: bool) -> AdapterError {
    let args = PressArgs {
        combo: "a".into(),
        app: Some("Editor".into()),
        window_id: None,
        force: false,
    };
    let context = CommandContext::default().with_headed(headed);
    match execute(args, &LayoutRefusingAdapter, &context) {
        Err(crate::AppError::Adapter(error)) => error,
        other => panic!("expected the adapter refusal, got {other:?}"),
    }
}

#[test]
fn a_key_refusal_after_the_headed_focus_records_the_focus() {
    let error = refusal(true);

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(error.moved_focus());
    assert_eq!(error.disposition, crate::DeliverySemantics::unknown());
}

#[test]
fn a_headless_key_refusal_records_no_focus() {
    assert!(!refusal(false).moved_focus());
}
