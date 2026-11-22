use super::*;
use crate::{ActionOps, ActionStep, Deadline, InputOps, ObservationOps, SystemOps};
use std::sync::atomic::{AtomicUsize, Ordering};

struct ShadowedField {
    execution: ActionResult,
    value: Option<&'static str>,
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
                value: self.value.map(str::to_string),
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
            value: Some("untitled folder"),
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

fn written_field(execution: ActionResult) -> (ShadowedField, Result<ActionResult, AdapterError>) {
    field_reading(execution, Some("untitled folder"))
}

fn field_reading(
    execution: ActionResult,
    value: Option<&'static str>,
) -> (ShadowedField, Result<ActionResult, AdapterError>) {
    let adapter = ShadowedField {
        execution,
        value,
        reads_after_dispatch: AtomicUsize::new(0),
        dispatched: AtomicUsize::new(0),
    };
    let result = execute_verified_action(
        &adapter,
        &NativeHandle::null(),
        ActionRequest::headless(Action::SetValue("untitled folder".into())),
        &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
    );
    (adapter, result)
}

#[test]
fn a_value_write_whose_commit_is_unobserved_reads_back_but_stays_unverified() {
    let steps = vec![ActionStep::succeeded("AXValue").with_verified(true)];
    let execution = ActionResult::from_execution(&Action::SetValue("x".into()), steps)
        .with_commit_unobserved("click another field");
    let (adapter, result) = written_field(execution);
    let result = result.unwrap();

    assert!(adapter.reads_after_dispatch.load(Ordering::SeqCst) > 0);
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
    let details = result.details.as_ref().unwrap();
    assert_eq!(details["application_commit"], "unobserved");
    assert_eq!(details["verification_reason"], "edit_commit_unobserved");
    assert_eq!(details["verification_scope"], "element_value");
    assert_eq!(details["commit_hint"], "click another field");
}

#[test]
fn an_unmarked_value_write_that_reads_back_stays_verified() {
    let steps = vec![ActionStep::succeeded("AXValue").with_verified(true)];
    let (_, result) = written_field(ActionResult::from_execution(
        &Action::SetValue("x".into()),
        steps,
    ));
    let result = result.unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(
        result.details.unwrap()["application_commit"],
        "not_verified"
    );
}

fn unobserved_write() -> ActionResult {
    let steps = vec![ActionStep::succeeded("AXValue").with_verified(true)];
    ActionResult::from_execution(&Action::SetValue("x".into()), steps)
        .with_commit_unobserved("focus another field")
}

#[test]
fn a_marked_value_write_that_did_not_land_still_fails_the_read_back() {
    let (_, result) = field_reading(unobserved_write(), Some("old text"));
    let error = result.expect_err("the field still holds its old text");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(
        error
            .message
            .contains("does not match the requested change"),
        "{}",
        error.message
    );
}

#[test]
fn a_marked_value_write_that_cannot_be_read_keeps_its_commit_report() {
    let (_, result) = field_reading(unobserved_write(), None);
    let result = result.unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
    let details = result.details.unwrap();
    assert_eq!(details["verification_scope"], "unavailable");
    assert_eq!(details["application_commit"], "unobserved");
    assert_eq!(details["commit_hint"], "focus another field");
}

fn committed_field(value: Option<&'static str>) -> Result<ActionResult, AdapterError> {
    let steps = vec![
        ActionStep::succeeded("AXValue").with_verified(true),
        ActionStep::succeeded("EndEditingByFocus").with_verified(true),
    ];
    let adapter = ShadowedField {
        execution: ActionResult::from_execution(&Action::SetValueAndCommit("0.3".into()), steps)
            .with_commit_ended(serde_json::json!({ "mechanism": "focus_round_trip" })),
        value,
        reads_after_dispatch: AtomicUsize::new(0),
        dispatched: AtomicUsize::new(0),
    };
    execute_verified_action(
        &adapter,
        &NativeHandle::null(),
        ActionRequest::headless(Action::SetValueAndCommit("0.3".into())),
        &InteractionLease::guarded(Deadline::after(1000).unwrap(), ()).unwrap(),
    )
}

#[test]
fn a_committed_write_the_app_kept_in_its_own_format_is_verified() {
    let result = committed_field(Some("0.3000")).expect("the app kept 0.3 as 0.3000");
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    let details = result.details.unwrap();
    assert_eq!(details["application_commit"], "committed");
    assert_eq!(details["commit_evidence"]["mechanism"], "focus_round_trip");
    assert_eq!(details["verification_scope"], "element_value");
}

#[test]
fn a_committed_write_the_app_did_not_keep_is_refused_with_a_next_step() {
    let error = committed_field(Some("0.2500")).expect_err("the app reverted the value");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(
        error.message.contains("did not keep the value"),
        "{}",
        error.message
    );
    assert!(error.message.contains("0.2500"), "{}", error.message);
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|hint| hint.contains("Write a value the field accepts")),
        "{:?}",
        error.suggestion
    );
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_committed_write_whose_value_cannot_be_read_is_not_reported_committed() {
    let result = committed_field(None).expect("an unreadable value is unverified, not a failure");
    let details = result.details.expect("details");
    assert_eq!(details["application_commit"], "not_verified");
    assert_ne!(details["delivery"], "delivered_verified");
}
