use crate::{
    Action, ActionOps, ActionRequest, ActionResult, AdapterError, CommandContext, Deadline,
    DeliverySemantics, ErrorCode, InputOps, InteractionLease, LiveElement, NativeHandle,
    ObservationOps, RefEntry, SystemOps,
    ref_action::{
        dispatch_resolved_landing, ref_action_context::RefActionContext,
        ref_action_wait_context::RefActionWaitContext,
    },
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct ModalAdapter {
    provisional_reads: AtomicUsize,
    full_reads: AtomicUsize,
    inputs: AtomicUsize,
}

impl ObservationOps for ModalAdapter {
    fn resolve_element_strict(
        &self,
        _: &RefEntry,
        _: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    crate::adapter::saved_ref_scope!("button", Some("Run"));

    fn get_live_element_provisional(
        &self,
        _: &NativeHandle,
        _: Deadline,
    ) -> Result<Option<LiveElement>, AdapterError> {
        self.provisional_reads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(LiveElement {
            identity: crate::adapter::live_identity("Run"),
            state: crate::ElementState {
                role: "button".into(),
                states: vec![],
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(crate::Rect {
                x: 1.0,
                y: 1.0,
                width: 20.0,
                height: 20.0,
            }),
            available_actions: vec![crate::actions::capability::CLICK.into()],
        }))
    }

    fn get_live_element(&self, _: &NativeHandle, _: Deadline) -> Result<LiveElement, AdapterError> {
        self.full_reads.fetch_add(1, Ordering::SeqCst);
        Err(
            AdapterError::new(ErrorCode::ActionFailed, "Blocked by modal")
                .with_details(serde_json::json!({"cause": "blocked_by_modal_dialog"}))
                .with_disposition(DeliverySemantics::not_delivered()),
        )
    }
}

impl ActionOps for ModalAdapter {
    fn execute_action(
        &self,
        _: &NativeHandle,
        _: ActionRequest,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.inputs.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for ModalAdapter {
    fn mouse_event(
        &self,
        _: crate::MouseEvent,
        _: &InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.inputs.fetch_add(1, Ordering::SeqCst);
        panic!("mouse input must not be sent")
    }

    fn key_event(
        &self,
        _: &crate::KeyCombo,
        _: bool,
        _: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.inputs.fetch_add(1, Ordering::SeqCst);
        panic!("keyboard input must not be sent")
    }
}

impl SystemOps for ModalAdapter {}

#[test]
fn resolved_dispatch_refuses_modal_after_provisional_read_without_sending_input() {
    let adapter = ModalAdapter::default();
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    let entry: RefEntry = serde_json::from_value(serde_json::json!({
        "pid": 1, "process_instance": "test-instance", "role": "button", "name": "Run",
        "source_window_id": "w-1", "source_surface": "window", "path": [0],
        "unique_at_capture": true, "bounds": bounds, "bounds_hash": bounds.bounds_hash(),
        "available_actions": ["Click"], "states": []
    }))
    .unwrap();
    let context = CommandContext::default();
    let deadline = Deadline::standard().unwrap();
    let target = RefActionContext::new(
        RefActionWaitContext {
            adapter: &adapter,
            entry: &entry,
            ref_id: "e1",
            context: &context,
        },
        deadline,
    );
    let lease = InteractionLease::guarded(deadline, ()).unwrap();
    let result = dispatch_resolved_landing(target, ActionRequest::headless(Action::Click), &lease);
    assert_eq!(
        adapter.inputs.load(Ordering::SeqCst),
        0,
        "no input may follow provisional-only evidence"
    );
    let error = result
        .err()
        .expect("modal must refuse dispatch")
        .into_adapter_error();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.details.unwrap()["cause"], "blocked_by_modal_dialog");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.provisional_reads.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.full_reads.load(Ordering::SeqCst), 1);
}
