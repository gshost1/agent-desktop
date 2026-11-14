//! The read of one child by position, bracketed by two counts.
use super::{
    child_read::{
        ChildRead, check_deadline, child_count, copy_page, is_absent_error, stable_count,
    },
    child_read_status::ChildReadStatus,
    child_read_telemetry as telemetry,
};
use crate::tree::AXElement;

pub(super) fn read_attribute_child_at(
    element: &AXElement,
    attribute: &str,
    index: usize,
    deadline: std::time::Instant,
) -> ChildRead {
    let mut status = ChildReadStatus::default();
    if check_deadline(deadline, &mut status).is_err() {
        return ChildRead::failed(status);
    }
    status.attempts += 1;
    let initial_count = match child_count(element, attribute, deadline) {
        Ok(count) => count,
        Err(error) if is_absent_error(error) => return ChildRead::unavailable(status),
        Err(error) => {
            telemetry::record(&mut status, attribute, "initial_count", error, None);
            return ChildRead::failed(status);
        }
    };
    let elements = if index < initial_count {
        status.attempts += 1;
        match copy_page(element, attribute, index, 1, deadline) {
            Ok(elements) => elements,
            Err(error) => {
                telemetry::record(
                    &mut status,
                    attribute,
                    "indexed_child",
                    error,
                    Some(initial_count),
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    if index < initial_count && elements.is_empty() {
        status.cursor_stalled = true;
    }
    let final_count = stable_count(element, attribute, deadline, &mut status);
    if let Err(error) = final_count {
        telemetry::record(
            &mut status,
            attribute,
            "stable_count",
            error,
            Some(initial_count),
        );
    }
    ChildRead::indexed(elements, initial_count, index, final_count, status)
}
