use super::{
    TextSelectionTarget, searchable_text, select_text_in, selected_text_matches, settled_selection,
    unique_range, utf16_occurrences,
};
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

type StubValue = Result<Option<(&'static str, bool)>, i32>;

struct StubField {
    value: StubValue,
    value_after_write: Option<StubValue>,
    selection_after_write: Option<std::ops::Range<usize>>,
    writes: std::cell::RefCell<Vec<std::ops::Range<usize>>>,
}

impl StubField {
    fn new(value: StubValue) -> Self {
        Self {
            value,
            value_after_write: None,
            selection_after_write: None,
            writes: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl TextSelectionTarget for StubField {
    fn accepts_selection(
        &self,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<bool, agent_desktop_core::AdapterError> {
        Ok(true)
    }

    fn read_value(
        &self,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<Option<BoundedString>, i32> {
        let current = match self.value_after_write {
            Some(after) if !self.writes.borrow().is_empty() => after,
            _ => self.value,
        };
        current.map(|value| {
            value.map(|(text, complete)| BoundedString {
                value: text.into(),
                complete,
            })
        })
    }

    fn write_selection(
        &self,
        range: &std::ops::Range<usize>,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<(), agent_desktop_core::AdapterError> {
        self.writes.borrow_mut().push(range.clone());
        Ok(())
    }

    fn read_selection(
        &self,
        _deadline: agent_desktop_core::Deadline,
    ) -> Option<std::ops::Range<usize>> {
        if self.writes.borrow().is_empty() {
            Some(0..0)
        } else {
            self.selection_after_write.clone()
        }
    }
}

fn select(field: &StubField, wanted: &str) -> Result<bool, agent_desktop_core::AdapterError> {
    select_text_in(
        field,
        wanted,
        agent_desktop_core::Deadline::after(5_000).unwrap(),
    )
}

#[test]
fn select_writes_the_one_occurrence_and_verifies_its_read_back() {
    let mut field = StubField::new(Ok(Some((OUTLINE, true))));
    field.selection_after_write = Some(15..19);

    assert!(select(&field, "Week").unwrap());
    assert_eq!(*field.writes.borrow(), vec![15..19]);
}

#[test]
fn select_fails_when_the_field_reads_back_another_selection() {
    let mut field = StubField::new(Ok(Some((OUTLINE, true))));
    field.selection_after_write = Some(0..0);

    let error = select(&field, "Week").unwrap_err();

    assert_eq!(*field.writes.borrow(), vec![15..19]);
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(error.details.unwrap()["kind"], "selection_not_observed");
}

#[test]
fn select_with_an_unreadable_selection_succeeds_unverified() {
    let field = StubField::new(Ok(Some((OUTLINE, true))));

    assert!(!select(&field, "Week").unwrap());
    assert_eq!(*field.writes.borrow(), vec![15..19]);
}

#[test]
fn select_refuses_a_failed_value_read_before_writing() {
    let field = StubField::new(Err(accessibility_sys::kAXErrorAPIDisabled));

    let error = select(&field, "Week").unwrap_err();

    assert!(field.writes.borrow().is_empty());
    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn select_refuses_a_truncated_value_before_writing() {
    let field = StubField::new(Ok(Some(("Course outline", false))));

    let error = select(&field, "Course").unwrap_err();

    assert!(field.writes.borrow().is_empty());
    assert_eq!(error.details.unwrap()["kind"], "text_value_truncated");
}

#[test]
fn select_fails_when_the_text_at_the_selected_range_changed() {
    let mut field = StubField::new(Ok(Some(("target rest", true))));
    field.value_after_write = Some(Ok(Some(("other! rest", true))));
    field.selection_after_write = Some(0..6);

    let error = select(&field, "target").unwrap_err();

    assert_eq!(*field.writes.borrow(), vec![0..6]);
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "selection_text_changed");
    assert_eq!(
        details["requested"],
        serde_json::json!({ "start": 0, "end": 6 })
    );
}

#[test]
fn select_with_an_unreadable_value_after_the_write_succeeds_unverified() {
    let mut field = StubField::new(Ok(Some(("target rest", true))));
    field.value_after_write = Some(Err(accessibility_sys::kAXErrorCannotComplete));
    field.selection_after_write = Some(0..6);

    assert!(!select(&field, "target").unwrap());
}

#[test]
fn select_with_a_truncated_value_after_the_write_succeeds_unverified() {
    let mut field = StubField::new(Ok(Some(("target rest", true))));
    field.value_after_write = Some(Ok(Some(("target", false))));
    field.selection_after_write = Some(0..6);

    assert!(!select(&field, "target").unwrap());
}

#[test]
fn select_verifies_when_only_text_outside_the_range_changed() {
    let mut field = StubField::new(Ok(Some(("target rest", true))));
    field.value_after_write = Some(Ok(Some(("target best", true))));
    field.selection_after_write = Some(0..6);

    assert!(select(&field, "target").unwrap());
}

#[test]
fn selected_text_is_compared_in_utf16_units() {
    assert!(selected_text_matches("café 😀 H2O", &(5..7), "😀").unwrap());
    assert!(selected_text_matches("café 😀 H2O", &(8..11), "H2O").unwrap());
    let error = selected_text_matches("café 😀 H2O", &(8..11), "H2").unwrap_err();
    assert_eq!(error.details.unwrap()["kind"], "selection_text_changed");
}

#[test]
fn a_range_past_the_changed_text_is_a_delivered_failure() {
    let error = selected_text_matches("short", &(10..16), "target").unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn overlapping_occurrences_are_counted() {
    assert_eq!(utf16_occurrences("banana", "ana"), vec![1..4, 3..6]);
    assert_eq!(utf16_occurrences("aaa", "aa"), vec![0..2, 1..3]);
    assert_eq!(utf16_occurrences("ééé", "éé"), vec![0..2, 1..3]);
    assert_eq!(utf16_occurrences("😀😀😀", "😀😀"), vec![0..4, 2..6]);
}

#[test]
fn an_overlapping_repeat_is_refused_before_writing() {
    let field = StubField::new(Ok(Some(("banana", true))));

    let error = select(&field, "ana").unwrap_err();

    assert!(field.writes.borrow().is_empty());
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    let details = error.details.unwrap();
    assert_eq!(details["occurrences"], 2);
    assert_eq!(details["offsets"], serde_json::json!([1, 3]));
}
