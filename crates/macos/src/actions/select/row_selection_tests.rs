use super::*;
use agent_desktop_core::DeliverySemantics;

fn uncertain_confirm() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "AXConfirm did not establish a verifiable effect",
    )
    .with_disposition(DeliverySemantics::uncertain())
}

#[test]
fn an_uncertain_click_that_selected_the_row_succeeds_with_the_postcondition() {
    let steps = settle_by_selection(Some(false), Err(uncertain_confirm()), |settle| {
        assert!(settle);
        true
    })
    .unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].label(), SELECTION_POSTCONDITION);
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn an_uncertain_click_without_a_selection_change_still_fails() {
    let error = settle_by_selection(Some(false), Err(uncertain_confirm()), |_| false).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
    );
}

#[test]
fn a_row_that_was_already_selected_cannot_confirm_the_click() {
    let mut read = false;
    let error = settle_by_selection(Some(true), Err(uncertain_confirm()), |_| {
        read = true;
        true
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(!read);
}

#[test]
fn a_target_without_readable_selection_keeps_the_chain_outcome() {
    let error = settle_by_selection(None, Err(uncertain_confirm()), |_| true).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
}

#[test]
fn an_undelivered_click_is_never_confirmed_by_selection() {
    let undelivered = AdapterError::new(ErrorCode::ActionFailed, "no step reached the app")
        .with_disposition(DeliverySemantics::not_delivered());
    let error = settle_by_selection(Some(false), Err(undelivered), |_| true).unwrap_err();
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn an_unverified_delivery_gains_the_postcondition_from_one_immediate_read() {
    let steps = settle_by_selection(
        Some(false),
        Ok(vec![ActionStep::succeeded("CGClick").with_verified(false)]),
        |settle| {
            assert!(!settle);
            true
        },
    )
    .unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[1].label(), SELECTION_POSTCONDITION);
}

#[test]
fn a_verified_delivery_skips_the_selection_read() {
    let steps = settle_by_selection(
        Some(false),
        Ok(vec![ActionStep::succeeded("AXPress").with_verified(true)]),
        |_| panic!("verified delivery must not read selection"),
    )
    .unwrap();
    assert_eq!(steps.len(), 1);
}
