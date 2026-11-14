use super::{unique_range, utf16_occurrences};
use agent_desktop_core::ErrorCode;

const OUTLINE: &str = "Course outline\nWeek one covers records.\nThe final week repeats weeks one to three.";

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
