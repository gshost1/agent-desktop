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
    for label in ["AXConfirm", "select_within_container"] {
        let step = settle_by_selection(Some(false), label, &Err(uncertain_confirm()), |settle| {
            assert!(settle);
            true
        })
        .unwrap();
        assert_eq!(step.label(), SELECTION_POSTCONDITION);
        assert_eq!(step.verified(), Some(true));
    }
}

#[test]
fn an_uncertain_click_without_a_selection_change_still_fails() {
    assert!(
        settle_by_selection(Some(false), "AXConfirm", &Err(uncertain_confirm()), |_| {
            false
        })
        .is_none()
    );
}

#[test]
fn a_row_that_was_already_selected_cannot_confirm_the_click() {
    let mut read = false;
    let step = settle_by_selection(Some(true), "AXConfirm", &Err(uncertain_confirm()), |_| {
        read = true;
        true
    });
    assert!(step.is_none());
    assert!(!read);
}

#[test]
fn a_target_without_readable_selection_keeps_the_chain_outcome() {
    assert!(settle_by_selection(None, "AXConfirm", &Err(uncertain_confirm()), |_| true).is_none());
}

#[test]
fn an_undelivered_click_is_never_confirmed_by_selection() {
    let undelivered = AdapterError::new(ErrorCode::ActionFailed, "no step reached the app")
        .with_disposition(DeliverySemantics::not_delivered());
    assert!(settle_by_selection(Some(false), "AXConfirm", &Err(undelivered), |_| true).is_none());
}

#[test]
fn an_unverified_delivery_gains_the_postcondition_from_one_immediate_read() {
    let step = settle_by_selection(
        Some(false),
        "CGClick",
        &Ok(DeliveryOutcome::DeliveredUnverified),
        |settle| {
            assert!(!settle);
            true
        },
    )
    .unwrap();
    assert_eq!(step.label(), SELECTION_POSTCONDITION);
}

#[test]
fn a_verified_delivery_skips_the_selection_read() {
    let step = settle_by_selection(
        Some(false),
        "AXConfirm",
        &Ok(DeliveryOutcome::DeliveredVerified),
        |_| panic!("verified delivery must not read selection"),
    );
    assert!(step.is_none());
}

#[test]
fn activation_is_not_confirmed_by_selection() {
    for label in ["AXOpen", "AXPress"] {
        let error = uncertain_confirm().with_details(serde_json::json!({"operation": label}));
        for (step, outcome) in [
            (label, Err(error.clone())),
            ("activate_descendant", Err(error)),
            (label, Ok(DeliveryOutcome::DeliveredUnverified)),
        ] {
            assert!(
                settle_by_selection(Some(false), step, &outcome, |_| {
                    panic!("activation must not read selection")
                })
                .is_none()
            );
        }
    }
}
