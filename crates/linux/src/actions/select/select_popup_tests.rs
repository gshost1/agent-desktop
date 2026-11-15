use std::cell::RefCell;

use agent_desktop_core::{ActionResult, AdapterError, DeliverySemantics, ErrorCode};

use super::{close_and_confirm, expand_then_choose, value_already_shown};

#[test]
fn a_drop_down_that_shows_the_wanted_value_needs_no_popup() {
    assert!(value_already_shown(Some(" gamma ".to_owned()), "Gamma"));
    assert!(!value_already_shown(Some("Beta".to_owned()), "Gamma"));
    assert!(!value_already_shown(None, "Gamma"));
}

fn missing_choice() -> AdapterError {
    AdapterError::new(ErrorCode::ElementNotFound, "no such choice")
        .with_details(serde_json::json!({ "searched": true }))
        .with_disposition(DeliverySemantics::not_delivered())
}

fn miss_after_expand(closed: bool) -> AdapterError {
    expand_then_choose(
        || Ok(()),
        || Err::<ActionResult, _>(missing_choice()),
        || closed,
    )
    .expect_err("the choice is missing")
}

#[test]
fn a_choice_missing_from_a_popup_read_back_collapsed_stays_unsent() {
    let error = miss_after_expand(true);
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["preparatory_expand"], "expanded_then_collapsed");
    assert_eq!(details["popup_closed"], true);
    assert_eq!(details["searched"], true);
}

#[test]
fn a_choice_missing_from_a_popup_that_may_still_be_open_is_not_untouched() {
    let error = miss_after_expand(false);
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["popup_closed"], false);
    assert!(details.get("preparatory_expand").is_none());
}

#[test]
fn a_failure_with_its_own_delivery_evidence_keeps_it_after_the_collapse() {
    for closed in [true, false] {
        let error = expand_then_choose(
            || Ok(()),
            || {
                Err::<ActionResult, _>(
                    AdapterError::new(ErrorCode::ActionFailed, "commit lost")
                        .with_disposition(DeliverySemantics::unknown()),
                )
            },
            || closed,
        )
        .expect_err("the commit fails");
        assert_eq!(error.disposition, DeliverySemantics::unknown());
        assert_eq!(error.details.expect("details")["popup_closed"], closed);
    }
}

#[test]
fn a_popup_that_refuses_to_expand_keeps_its_own_disposition() {
    let mut chose = false;
    let error = expand_then_choose(
        || Err(missing_choice()),
        || {
            chose = true;
            Ok(ActionResult::satisfied_without_delivery("select"))
        },
        || true,
    )
    .expect_err("the expand is refused");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(
        error
            .details
            .expect("details")
            .get("popup_closed")
            .is_none()
    );
    assert!(!chose);
}

fn confirm(reads: Vec<Option<bool>>, collapse: Result<(), AdapterError>) -> (bool, bool) {
    let reads = RefCell::new(reads.into_iter());
    let collapsed = RefCell::new(false);
    let closed = close_and_confirm(
        || reads.borrow_mut().next().flatten(),
        || {
            *collapsed.borrow_mut() = true;
            collapse
        },
    );
    (closed, collapsed.into_inner())
}

#[test]
fn a_popup_already_read_collapsed_is_closed_without_a_collapse() {
    assert_eq!(confirm(vec![Some(false)], Ok(())), (true, false));
}

#[test]
fn an_open_popup_is_closed_only_when_a_read_after_the_collapse_shows_it_collapsed() {
    assert_eq!(confirm(vec![Some(true), Some(false)], Ok(())), (true, true));
    assert_eq!(confirm(vec![Some(true), Some(true)], Ok(())), (false, true));
    assert_eq!(confirm(vec![Some(true), None], Ok(())), (false, true));
}

#[test]
fn a_collapse_that_errors_leaves_the_popup_not_closed() {
    let refused = AdapterError::new(ErrorCode::ActionFailed, "collapse refused");
    assert_eq!(
        confirm(vec![Some(true), Some(false)], Err(refused)),
        (false, true)
    );
}

#[test]
fn an_unreadable_popup_state_is_not_closed() {
    assert_eq!(confirm(vec![None], Ok(())), (false, false));
}
