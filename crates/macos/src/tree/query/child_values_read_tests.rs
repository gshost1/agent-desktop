use super::super::child_read_telemetry::record_status as record_error;
use super::*;
use accessibility_sys::{
    kAXErrorAPIDisabled, kAXErrorAttributeUnsupported, kAXErrorCannotComplete,
    kAXErrorIllegalArgument, kAXErrorInvalidUIElement, kAXErrorNoValue,
};

#[test]
fn a_page_shorter_than_requested_is_the_whole_list() {
    assert_eq!(
        first_page_outcome(&Ok(vec![0, 1, 2]), 128),
        FirstPageOutcome::Whole
    );
    assert_eq!(
        first_page_outcome(&Ok(vec![0, 1]), 5),
        FirstPageOutcome::Whole
    );
}

#[test]
fn a_full_page_goes_to_the_counted_read() {
    assert_eq!(
        first_page_outcome(&Ok(vec![0; 5]), 5),
        FirstPageOutcome::Counted,
        "a full page may have a tail that only a count can reveal"
    );
    assert_eq!(
        first_page_outcome(&Ok(vec![0; 128]), 128),
        FirstPageOutcome::Counted
    );
}

#[test]
fn an_empty_or_unsupported_list_goes_to_the_counted_read() {
    for error in [
        kAXErrorIllegalArgument,
        kAXErrorAttributeUnsupported,
        kAXErrorNoValue,
        i32::MIN,
    ] {
        assert_eq!(
            first_page_outcome::<u8>(&Err(error), 128),
            FirstPageOutcome::Counted,
            "error {error} must be decided by the counted read"
        );
    }
}

#[test]
fn errors_the_counted_read_would_meet_again_end_the_attempt() {
    for error in [
        kAXErrorInvalidUIElement,
        kAXErrorAPIDisabled,
        kAXErrorCannotComplete,
    ] {
        assert_eq!(
            first_page_outcome::<u8>(&Err(error), 128),
            FirstPageOutcome::Terminal(error)
        );
    }
}

#[test]
fn only_container_roles_read_values_first() {
    for role in ["AXGroup", "AXCell", "AXRow", "AXScrollArea", "AXWindow"] {
        assert!(holds_children_by_role(Some(role)), "{role}");
    }
    for role in [
        "AXStaticText",
        "AXButton",
        "AXTextField",
        "AXImage",
        "AXCheckBox",
    ] {
        assert!(
            !holds_children_by_role(Some(role)),
            "{role} is usually a leaf, where one count is cheaper than a refused page and a count"
        );
    }
    assert!(!holds_children_by_role(None));
}

#[test]
fn a_read_with_no_element_budget_never_tries_the_first_page() {
    let mut status = ChildReadStatus::default();
    let read = read_short_first_page(
        &AXElement(std::ptr::null_mut()),
        "AXChildren",
        0,
        std::time::Instant::now() + std::time::Duration::from_secs(1),
        &mut status,
    );

    assert!(read.is_none());
    assert_eq!(status.attempts, 0);
}

#[test]
fn an_invalid_element_ends_the_first_page_read_without_a_count() {
    let mut status = ChildReadStatus::default();
    let read = read_short_first_page(
        &AXElement(std::ptr::null_mut()),
        "AXChildren",
        4_096,
        std::time::Instant::now() + std::time::Duration::from_secs(1),
        &mut status,
    )
    .expect("an invalid element is terminal for this attempt");

    assert!(read.status.invalid_element);
    assert_eq!(read.status.attempts, 1);
    assert!(!read.complete);
}

#[test]
fn count_fallback_preserves_a_terminal_prefix_failure() {
    let mut status = ChildReadStatus {
        attempts: 1,
        ..Default::default()
    };
    record_error(&mut status, kAXErrorCannotComplete);
    let read = read_without_count(
        &AXElement(std::ptr::null_mut()),
        "AXChildren",
        1,
        std::time::Instant::now() + std::time::Duration::from_secs(1),
        status,
    );

    assert!(read.status.invalid_element);
    assert_eq!(read.status.health.cannot_complete, 1);
    assert_eq!(read.status.attempts, 2);
    assert!(!read.complete);
    assert!(!read.prefix_certain);
}
