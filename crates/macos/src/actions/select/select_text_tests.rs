use super::{searchable_text, settled_selection, unique_range, utf16_occurrences};
use crate::tree::properties::bounded_string::BoundedString;
use agent_desktop_core::{DeliverySemantics, ErrorCode};

const OUTLINE: &str =
    "Course outline\nWeek one covers records.\nThe final week repeats weeks one to three.";

#[test]
fn a_text_that_occurs_once_selects_its_exact_range() {
    let range = unique_range(OUTLINE, "The final week repeats weeks one to three.").unwrap();

    assert_eq!(range, 40..82);
    assert_eq!(range.end, OUTLINE.encode_utf16().count());
}

#[test]
fn ranges_count_utf16_units_not_bytes() {
    let text = "café 😀 H2O";

    assert_eq!(utf16_occurrences(text, "H2O"), vec![8..11]);
    assert_eq!(utf16_occurrences(text, "😀"), vec![5..7]);
}

#[test]
fn a_repeated_text_is_refused_with_every_offset() {
    let error = unique_range("H2O here, H2O there", "H2O").unwrap_err();

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    let details = error.details.unwrap();
    assert_eq!(details["occurrences"], 2);
    assert_eq!(details["offsets"], serde_json::json!([0, 10]));
}

#[test]
fn a_missing_text_is_refused_as_not_found() {
    let error = unique_range(OUTLINE, "week Four").unwrap_err();

    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(error.details.unwrap()["kind"], "text_not_found");
}

#[test]
fn matching_is_case_sensitive() {
    assert!(unique_range(OUTLINE, "the final week").is_err());
    assert!(unique_range(OUTLINE, "The final week").is_ok());
}

#[test]
fn an_empty_text_is_an_argument_error() {
    assert_eq!(
        unique_range(OUTLINE, "").unwrap_err().code,
        ErrorCode::InvalidArgs
    );
}

#[test]
fn a_selection_read_back_as_another_range_is_a_failed_delivery() {
    let error = settled_selection(Some(0..0), &(40..46)).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "selection_not_observed");
    assert_eq!(
        details["requested"],
        serde_json::json!({ "start": 40, "end": 46 })
    );
    assert_eq!(
        details["observed"],
        serde_json::json!({ "start": 0, "end": 0 })
    );
}

#[test]
fn an_unreadable_selection_stays_delivered_unverified() {
    assert!(!settled_selection(None, &(40..46)).unwrap());
}

#[test]
fn the_requested_selection_read_back_is_verified() {
    assert!(settled_selection(Some(40..46), &(40..46)).unwrap());
}

#[test]
fn a_text_value_cut_by_the_observation_budget_is_refused_before_writing() {
    let read = Ok(Some(BoundedString {
        value: "Course outline".into(),
        complete: false,
    }));

    let error = searchable_text(read).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.details.unwrap()["kind"], "text_value_truncated");
}

#[test]
fn a_failed_value_read_surfaces_as_its_own_error_before_writing() {
    let cases = [
        (
            accessibility_sys::kAXErrorAPIDisabled,
            ErrorCode::PermDenied,
        ),
        (
            accessibility_sys::kAXErrorInvalidUIElement,
            ErrorCode::StaleRef,
        ),
        (
            accessibility_sys::kAXErrorCannotComplete,
            ErrorCode::AppUnresponsive,
        ),
        (accessibility_sys::kAXErrorFailure, ErrorCode::ActionFailed),
    ];
    for (ax_error, code) in cases {
        let error = searchable_text(Err(ax_error)).unwrap_err();

        assert_eq!(error.code, code, "ax error {ax_error}");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "text_value_unreadable");
        assert_eq!(details["ax_error"], ax_error);
    }
}

#[test]
fn a_whole_or_absent_value_is_searched_as_read() {
    let whole = Ok(Some(BoundedString {
        value: OUTLINE.into(),
        complete: true,
    }));

    assert_eq!(searchable_text(whole).unwrap(), OUTLINE);
    assert_eq!(searchable_text(Ok(None)).unwrap(), "");
}
