use super::*;
use crate::{ActionOps, ActionStep, Deadline, InputOps, ObservationOps, SystemOps};
use std::sync::atomic::{AtomicUsize, Ordering};

struct ShadowedField {
    execution: ActionResult,
    reads_after_dispatch: AtomicUsize,
    dispatched: AtomicUsize,
}

impl ObservationOps for ShadowedField {
    fn get_live_element(&self, _: &NativeHandle, _: Deadline) -> Result<LiveElement, AdapterError> {
        if self.dispatched.load(Ordering::SeqCst) > 0 {
            self.reads_after_dispatch.fetch_add(1, Ordering::SeqCst);
        }
        Ok(LiveElement {
            identity: crate::adapter::live_identity("name"),
            state: crate::ElementState {
                role: "textfield".into(),
                states: vec![],
                value: Some("untitled folder".into()),
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: None,
            available_actions: vec![],
        })
    }

    fn get_text_selection(
        &self,
        _: &NativeHandle,
        _: Deadline,
    ) -> Result<Option<std::ops::Range<usize>>, AdapterError> {
        Ok(None)
    }
}

impl ActionOps for ShadowedField {
    fn execute_action(
        &self,
        _: &NativeHandle,
        _: ActionRequest,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatched.fetch_add(1, Ordering::SeqCst);
        Ok(self.execution.clone())
    }
}

impl InputOps for ShadowedField {}
impl SystemOps for ShadowedField {}

#[test]
fn text_held_by_an_inline_editor_is_unverified_and_names_the_commit_key() {
    for action in [
        Action::SetValue("renamed".into()),
        Action::TypeText("renamed".into()),
    ] {
        let adapter = ShadowedField {
            execution: ActionResult::awaiting_commit(
                &action,
                vec![ActionStep::succeeded("AXValue").with_verified(true)],
                "return",
            ),
            reads_after_dispatch: AtomicUsize::new(0),
            dispatched: AtomicUsize::new(0),
        };

        let result = execute_verified_action(
            &adapter,
            &NativeHandle::null(),
            ActionRequest::headless(action),
            &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
        )
        .expect("the field underneath an editor keeps its old value until commit");

        assert_eq!(
            result.disposition(),
            DeliverySemantics::delivered_unverified()
        );
        let details = result.details.as_ref().unwrap();
        assert_eq!(details["application_commit"], "pending");
        assert_eq!(details["verification_scope"], "inline_editor");
        assert_eq!(details["commit_key"], "return");
        assert_eq!(adapter.reads_after_dispatch.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn verified_steps_alone_never_mark_a_pending_commit() {
    let result = ActionResult::from_execution(
        &Action::SetValue("renamed".into()),
        vec![ActionStep::succeeded("AXValue").with_verified(true)],
    );

    assert!(!result.awaits_application_commit());
    assert!(
        ActionResult::awaiting_commit(&Action::SetValue("x".into()), vec![], "return")
            .awaits_application_commit()
    );
}
