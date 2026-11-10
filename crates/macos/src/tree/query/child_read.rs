use crate::tree::AXElement;

use super::{
    child_page::read_paged_prefix, child_read_status::ChildReadStatus,
    child_source_availability::ChildSourceAvailability,
};

pub(crate) struct ChildRead {
    pub(crate) elements: Vec<AXElement>,
    pub(crate) total_count: usize,
    pub(crate) complete: bool,
    pub(crate) source_availability: ChildSourceAvailability,
    pub(crate) prefix_certain: bool,
    pub(crate) status: ChildReadStatus,
}

impl ChildRead {
    pub(crate) fn empty(complete: bool) -> Self {
        Self {
            elements: Vec::new(),
            total_count: 0,
            complete,
            source_availability: if complete {
                ChildSourceAvailability::Available
            } else {
                ChildSourceAvailability::Unknown
            },
            prefix_certain: complete,
            status: ChildReadStatus::default(),
        }
    }

    pub(super) fn failed(status: ChildReadStatus) -> Self {
        Self {
            elements: Vec::new(),
            total_count: 0,
            complete: false,
            source_availability: ChildSourceAvailability::Unknown,
            prefix_certain: false,
            status,
        }
    }

    pub(super) fn unavailable(status: ChildReadStatus) -> Self {
        Self {
            elements: Vec::new(),
            total_count: 0,
            complete: true,
            source_availability: ChildSourceAvailability::Unavailable,
            prefix_certain: true,
            status,
        }
    }

    pub(super) fn indexed(
        mut elements: Vec<AXElement>,
        initial_count: usize,
        index: usize,
        final_count: Result<usize, i32>,
        mut status: ChildReadStatus,
    ) -> Self {
        let Ok(final_count) = final_count else {
            return Self::failed(status);
        };
        status.count_changed |= initial_count != final_count;
        let expected = usize::from(index < initial_count);
        let complete = !status.count_changed && elements.len() == expected;
        elements.truncate(expected);
        Self {
            elements,
            total_count: final_count,
            complete,
            source_availability: ChildSourceAvailability::Available,
            prefix_certain: complete,
            status,
        }
    }

    pub(crate) fn truncated(&self) -> bool {
        self.elements.len() < self.total_count
    }
}

mod imp {
    use super::super::{child_read_budget::ChildReadBudget, child_read_telemetry as telemetry};
    use super::*;
    use crate::{tree::properties::ax_value, tree::properties::cf_type::created_cf_array};
    use accessibility_sys::{kAXErrorAttributeUnsupported, kAXErrorNoValue, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};
    use core_foundation_sys::base::{CFIndex, CFTypeRef};

    pub(in crate::tree::query) const CHILD_PAGE_SIZE: usize = 128;

    pub(crate) fn read_children(
        element: &AXElement,
        role: Option<&str>,
        budget: ChildReadBudget,
        deadline: std::time::Instant,
    ) -> ChildRead {
        super::super::child_source::read_resilient(
            crate::tree::properties::element::child_attributes(role),
            deadline,
            |attribute| {
                read_attribute_children_with(
                    element,
                    attribute,
                    budget,
                    deadline,
                    super::super::child_values_read::holds_children_by_role(role),
                )
            },
        )
    }

    pub(crate) fn read_child_at(
        element: &AXElement,
        role: Option<&str>,
        index: usize,
        deadline: std::time::Instant,
    ) -> ChildRead {
        super::super::child_source::read_resilient(
            crate::tree::properties::element::child_attributes(role),
            deadline,
            |attribute| {
                super::super::child_read_at::read_attribute_child_at(
                    element, attribute, index, deadline,
                )
            },
        )
    }

    pub(crate) fn read_attribute_children(
        element: &AXElement,
        attribute: &str,
        budget: impl Into<ChildReadBudget>,
        deadline: std::time::Instant,
    ) -> ChildRead {
        read_attribute_children_with(element, attribute, budget.into(), deadline, false)
    }

    fn read_attribute_children_with(
        element: &AXElement,
        attribute: &str,
        budget: ChildReadBudget,
        deadline: std::time::Instant,
        values_first: bool,
    ) -> ChildRead {
        let (max_elements, boundary) = (budget.max_elements, budget.boundary);
        let mut status = ChildReadStatus::default();
        if check_deadline(deadline, &mut status).is_err() {
            return ChildRead::failed(status);
        }
        if values_first {
            if let Some(read) = super::super::child_values_read::read_short_first_page(
                element,
                attribute,
                max_elements,
                deadline,
                &mut status,
            ) {
                return read;
            }
        }
        let count_deadline = super::super::child_read_budget::count_deadline(boundary, deadline);
        status.attempts += 1;
        let count = match child_count(element, attribute, count_deadline) {
            Ok(count) => count,
            Err(error) if is_absent_error(error) => {
                return ChildRead::unavailable(status);
            }
            Err(error) => {
                telemetry::record(&mut status, attribute, "initial_count", error, None);
                return super::super::child_values_read::read_without_count(
                    element,
                    attribute,
                    max_elements,
                    deadline,
                    status,
                );
            }
        };
        let requested = count.min(max_elements);
        let whole_page =
            super::super::child_read_budget::whole_list_page(count, max_elements, CHILD_PAGE_SIZE);
        let mut complete = true;
        let read = whole_page
            .and_then(|page_len| {
                super::super::child_values_read::read_whole_page(
                    element,
                    attribute,
                    page_len,
                    deadline,
                    &mut status,
                )
            })
            .unwrap_or_else(|| read_prefix(element, attribute, requested, deadline, &mut status));
        let mut elements = match read {
            Ok(elements) => elements,
            Err(error) => {
                telemetry::record(&mut status, attribute, "prefix", error, Some(count));
                complete = false;
                Vec::new()
            }
        };
        let proven = whole_page.is_some() && elements.len() == count;
        elements.truncate(requested);
        complete &= elements.len() == requested;
        let recount = (super::super::child_read_budget::recount_after_prefix(requested) && !proven)
            .then(|| stable_count(element, attribute, deadline, &mut status));
        let final_count = match recount.unwrap_or(Ok(count)) {
            Ok(final_count) => final_count,
            Err(error) => {
                telemetry::record(&mut status, attribute, "stable_count", error, Some(count));
                complete = false;
                count
            }
        };
        if count != final_count {
            status.count_changed = true;
            complete = false;
            elements.truncate(final_count);
        }
        ChildRead {
            elements,
            total_count: final_count,
            complete,
            source_availability: ChildSourceAvailability::Available,
            prefix_certain: complete,
            status,
        }
    }

    pub(in crate::tree::query) fn child_count(
        element: &AXElement,
        attribute: &str,
        deadline: std::time::Instant,
    ) -> Result<usize, i32> {
        let attribute = CFString::new(attribute);
        crate::tree::properties::ax_ipc::attribute_value_count(
            element,
            attribute.as_concrete_TypeRef(),
            deadline,
        )
    }

    pub(in crate::tree::query) fn read_prefix(
        element: &AXElement,
        attribute: &str,
        requested: usize,
        deadline: std::time::Instant,
        status: &mut ChildReadStatus,
    ) -> Result<Vec<AXElement>, i32> {
        let result = read_paged_prefix(requested, CHILD_PAGE_SIZE, |index, page_len| {
            check_deadline(deadline, status)
                .map_err(|_| accessibility_sys::kAXErrorCannotComplete)?;
            status.attempts += 1;
            copy_page(element, attribute, index, page_len, deadline)
        })?;
        status.cursor_stalled |= result.stalled;
        Ok(result.elements)
    }

    pub(in crate::tree::query) fn stable_count(
        element: &AXElement,
        attribute: &str,
        deadline: std::time::Instant,
        status: &mut ChildReadStatus,
    ) -> Result<usize, i32> {
        check_deadline(deadline, status).map_err(|_| accessibility_sys::kAXErrorCannotComplete)?;
        status.attempts += 1;
        child_count(element, attribute, deadline)
    }

    pub(in crate::tree::query) fn copy_page(
        element: &AXElement,
        attribute: &str,
        index: usize,
        max_values: usize,
        deadline: std::time::Instant,
    ) -> Result<Vec<AXElement>, i32> {
        let attribute = CFString::new(attribute);
        let index = CFIndex::try_from(index).map_err(|_| i32::MIN)?;
        let max_values = CFIndex::try_from(max_values).map_err(|_| i32::MIN)?;
        let (error, result) = crate::tree::properties::ax_ipc::copy_attribute_values(
            element,
            attribute.as_concrete_TypeRef(),
            index,
            max_values,
            deadline,
        );
        if error != kAXErrorSuccess {
            if !result.is_null() {
                drop(created_cf_array(result as CFTypeRef));
            }
            return Err(error);
        }
        if result.is_null() {
            return Ok(Vec::new());
        }
        let Some(array) = created_cf_array(result as CFTypeRef) else {
            return Err(i32::MIN);
        };
        let expected = array.len() as usize;
        let elements = array
            .into_iter()
            .filter_map(|value| ax_value::retained_ax_element(&value))
            .collect::<Vec<_>>();
        if elements.len() != expected {
            return Err(i32::MIN);
        }
        Ok(elements)
    }

    pub(in crate::tree::query) fn check_deadline(
        deadline: std::time::Instant,
        status: &mut ChildReadStatus,
    ) -> Result<(), ()> {
        crate::tree::locator_deadline::remaining(deadline)
            .map(|_| ())
            .map_err(|_| {
                status.health.deadline_exhausted = 1;
            })
    }

    pub(in crate::tree::query) fn is_absent_error(error: i32) -> bool {
        error == kAXErrorAttributeUnsupported || error == kAXErrorNoValue
    }
}

pub(in crate::tree::query) use imp::{
    CHILD_PAGE_SIZE, check_deadline, child_count, copy_page, is_absent_error, read_prefix,
    stable_count,
};
pub(crate) use imp::{read_attribute_children, read_child_at, read_children};

#[cfg(test)]
#[path = "child_read_tests.rs"]
mod tests;
