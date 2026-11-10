//! Child value reads that replace, finish or stand in for a count.
#![cfg(target_os = "macos")]

use super::{
    child_read::{CHILD_PAGE_SIZE, ChildRead, check_deadline, copy_page, read_prefix},
    child_read_status::ChildReadStatus,
    child_read_telemetry as telemetry,
    child_source_availability::ChildSourceAvailability,
};
use crate::tree::AXElement;
use accessibility_sys::{kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorInvalidUIElement};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FirstPageOutcome {
    Whole,
    Counted,
    Terminal(i32),
}

/// Roles that exist to hold other elements. Their child count is where a
/// responder builds the child list, so asking for the values first spares
/// that work twice; an empty one costs one cheap extra call. Every other role
/// is usually a leaf, where a count alone is the cheapest answer.
pub(crate) fn holds_children_by_role(role: Option<&str>) -> bool {
    matches!(
        role,
        Some(
            "AXGroup"
                | "AXCell"
                | "AXRow"
                | "AXScrollArea"
                | "AXSplitGroup"
                | "AXToolbar"
                | "AXOutline"
                | "AXTable"
                | "AXList"
                | "AXWindow"
                | "AXSheet"
                | "AXTabGroup"
                | "AXRadioGroup"
                | "AXLayoutArea"
                | "AXWebArea"
        )
    )
}

/// A page shorter than requested is the responder's whole list, read at one
/// moment. A full page may have a tail, and an empty list reaches us as an
/// error (AppKit answers a zero-length range with an illegal-argument error),
/// so both go to the counted read. Errors that the counted read would only
/// meet again end the attempt.
pub(crate) fn first_page_outcome<T>(
    page: &Result<Vec<T>, i32>,
    page_len: usize,
) -> FirstPageOutcome {
    match page {
        Ok(elements) if elements.len() < page_len => FirstPageOutcome::Whole,
        Ok(_) => FirstPageOutcome::Counted,
        Err(error)
            if [
                kAXErrorInvalidUIElement,
                kAXErrorAPIDisabled,
                kAXErrorCannotComplete,
            ]
            .contains(error) =>
        {
            FirstPageOutcome::Terminal(*error)
        }
        Err(_) => FirstPageOutcome::Counted,
    }
}

/// One values read answers a container's child read without the count that
/// would otherwise precede it. `None` hands the read to the counted path,
/// which then decides exactly as it would have without this read.
pub(super) fn read_short_first_page(
    element: &AXElement,
    attribute: &str,
    max_elements: usize,
    deadline: std::time::Instant,
    status: &mut ChildReadStatus,
) -> Option<ChildRead> {
    if max_elements == 0 {
        return None;
    }
    let page_len = max_elements.min(CHILD_PAGE_SIZE);
    status.attempts += 1;
    let page = copy_page(element, attribute, 0, page_len, deadline);
    match first_page_outcome(&page, page_len) {
        FirstPageOutcome::Whole => {
            let elements = page.unwrap_or_default();
            Some(ChildRead {
                total_count: elements.len(),
                elements,
                complete: true,
                source_availability: ChildSourceAvailability::Available,
                prefix_certain: true,
                status: std::mem::take(status),
            })
        }
        FirstPageOutcome::Terminal(error) => {
            telemetry::record(status, attribute, "first_page", error, None);
            Some(ChildRead::failed(std::mem::take(status)))
        }
        FirstPageOutcome::Counted => None,
    }
}

/// A page that asks for one child more than the count and gets exactly the
/// count is the whole list at one moment, which is what the second count
/// would otherwise have to prove. `None` when the responder refuses the longer
/// range, so the caller reads the exact range instead.
pub(super) fn read_whole_page(
    element: &AXElement,
    attribute: &str,
    page_len: usize,
    deadline: std::time::Instant,
    status: &mut ChildReadStatus,
) -> Option<Result<Vec<AXElement>, i32>> {
    if check_deadline(deadline, status).is_err() {
        return Some(Err(accessibility_sys::kAXErrorCannotComplete));
    }
    status.attempts += 1;
    match copy_page(element, attribute, 0, page_len, deadline) {
        Err(accessibility_sys::kAXErrorIllegalArgument) => None,
        page => Some(page),
    }
}

/// A responder can serve the values of an attribute while failing to answer
/// its count inside the count budget — Xcode does this for `AXWindows`. The
/// count is only an optimisation, so a short read still proves the elements
/// it returned; only a full page leaves the tail unknown.
pub(super) fn read_without_count(
    element: &AXElement,
    attribute: &str,
    max_elements: usize,
    deadline: std::time::Instant,
    mut status: ChildReadStatus,
) -> ChildRead {
    if max_elements == 0 {
        return ChildRead::failed(status);
    }
    let elements = match read_prefix(element, attribute, max_elements, deadline, &mut status) {
        Ok(elements) => elements,
        Err(error) => {
            telemetry::record(&mut status, attribute, "prefix", error, None);
            return ChildRead::failed(status);
        }
    };
    let saturated = elements.len() == max_elements;
    ChildRead {
        total_count: elements.len() + usize::from(saturated),
        elements,
        complete: !saturated,
        source_availability: ChildSourceAvailability::Available,
        prefix_certain: !saturated,
        status,
    }
}

#[cfg(test)]
#[path = "child_values_read_tests.rs"]
mod tests;
