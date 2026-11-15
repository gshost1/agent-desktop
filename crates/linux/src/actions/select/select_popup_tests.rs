use agent_desktop_core::{ActionResult, AdapterError, DeliverySemantics, ErrorCode};

use super::{expand_then_choose, value_already_shown};

#[test]
fn a_drop_down_that_shows_the_wanted_value_needs_no_popup() {
    assert!(value_already_shown(Some(" gamma ".to_owned()), "Gamma"));
    assert!(!value_already_shown(Some("Beta".to_owned()), "Gamma"));
    assert!(!value_already_shown(None, "Gamma"));
}

fn missing_choice() -> AdapterError {
    AdapterError::new(ErrorCode::ElementNotFound, "no such choice")
        .with_disposition(DeliverySemantics::not_delivered())
}

#[test]
fn a_choice_missing_from_an_expanded_popup_is_not_reported_as_untouched() {
    let mut closed = false;
    let error = expand_then_choose(
        || Ok(()),
        || Err::<ActionResult, _>(missing_choice()),
        || closed = true,
    )
    .expect_err("the choice is missing");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert!(closed);
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
        || {},
    )
    .expect_err("the expand is refused");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(!chose);
}
