use super::{edits_through_focus, settle_focus_failure};
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

#[test]
fn text_fields_and_combo_boxes_take_focus_before_a_value_write() {
    assert!(edits_through_focus(Some("textfield")));
    assert!(edits_through_focus(Some("combobox")));
}

#[test]
fn other_value_controls_are_written_without_moving_focus() {
    for role in ["slider", "stepper", "checkbox", "incrementor", "button"] {
        assert!(!edits_through_focus(Some(role)), "{role}");
    }
    assert!(!edits_through_focus(None));
}

fn refused(code: ErrorCode, disposition: DeliverySemantics) -> AdapterError {
    AdapterError::new(code, "focus refused").with_disposition(disposition)
}

#[test]
fn a_lost_permission_or_vanished_field_ends_the_write_at_the_focus_step() {
    for code in [
        ErrorCode::PermDenied,
        ErrorCode::StaleRef,
        ErrorCode::ElementNotFound,
        ErrorCode::AppNotFound,
        ErrorCode::AppUnresponsive,
    ] {
        let error = settle_focus_failure(
            refused(code.clone(), DeliverySemantics::not_delivered()),
            false,
        )
        .expect_err("the write cannot go on");
        assert_eq!(error.code, code);
    }
}

#[test]
fn a_focus_failure_after_the_deadline_ends_the_write() {
    let refusal = refused(ErrorCode::ActionFailed, DeliverySemantics::not_delivered());
    assert!(settle_focus_failure(refusal, true).is_err());
}

#[test]
fn a_declined_focus_leaves_a_later_refusal_undelivered() {
    let refusal = refused(ErrorCode::ActionFailed, DeliverySemantics::not_delivered());
    let focus = settle_focus_failure(refusal, false).expect("the write goes on");
    let later = refused(ErrorCode::ActionFailed, DeliverySemantics::not_delivered());
    assert_eq!(
        focus.annotate(later).disposition,
        DeliverySemantics::not_delivered()
    );
}

#[test]
fn an_uncertain_focus_makes_a_later_refusal_delivered_unverified() {
    let refusal = refused(ErrorCode::ActionFailed, DeliverySemantics::uncertain());
    let focus = settle_focus_failure(refusal, false).expect("the write goes on");
    let later = refused(ErrorCode::ActionFailed, DeliverySemantics::not_delivered());
    assert_eq!(
        focus.annotate(later).disposition,
        DeliverySemantics::delivered_unverified()
    );
}
