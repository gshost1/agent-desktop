//! Role and frame reads for the scroll signals, one
//! `AXUIElementCopyMultipleAttributeValues` call per element instead of one
//! call per attribute. A read that fails for any reason other than the
//! attribute's absence is unreadable, never an absent frame.

use agent_desktop_core::{Deadline, Rect};

use crate::tree::AXElement;

#[derive(Default)]
pub(super) struct ElementRead {
    pub(super) role: Option<String>,
    pub(super) frame: Option<Rect>,
}

pub(super) fn read_frame(element: &AXElement, deadline: Deadline) -> Result<Option<Rect>, ()> {
    read(element, &["AXPosition", "AXSize"], deadline).map(|read| read.frame)
}

pub(super) fn read_role_and_frame(
    element: &AXElement,
    deadline: Deadline,
) -> Result<ElementRead, ()> {
    read(element, &["AXRole", "AXPosition", "AXSize"], deadline)
}

fn read(element: &AXElement, attributes: &[&str], deadline: Deadline) -> Result<ElementRead, ()> {
    use core_foundation::{array::CFArray, base::TCFType, string::CFString};

    let instant = crate::tree::locator_deadline::from_operation(deadline).map_err(|_| ())?;
    let names = attributes
        .iter()
        .map(|name| CFString::new(name))
        .collect::<Vec<_>>();
    let refs = names
        .iter()
        .map(|name| name.as_concrete_TypeRef())
        .collect::<Vec<_>>();
    let requested = CFArray::from_copyable(&refs);
    let (error, values) = crate::tree::properties::ax_ipc::copy_multiple_attribute_values(
        element,
        requested.as_concrete_TypeRef(),
        instant,
    );
    let values = crate::tree::properties::cf_type::created_cf_array(values);
    deadline.ensure_remaining().map_err(|_| ())?;
    if error != accessibility_sys::kAXErrorSuccess {
        return crate::tree::properties::ax_absence::is_absent_attribute_error(error)
            .then(ElementRead::default)
            .ok_or(());
    }
    let Some(values) = values else {
        return Ok(ElementRead::default());
    };
    let slots = values
        .iter()
        .map(|item| slot(&item))
        .collect::<Result<Vec<_>, ()>>()?;
    let frame_at = attributes.len().checked_sub(2).ok_or(())?;
    let role = (frame_at == 1)
        .then(|| slots.first().cloned().flatten())
        .flatten()
        .and_then(|item| {
            crate::tree::properties::cf_type::borrowed_cf_string(item.as_CFTypeRef())
                .map(|role| role.to_string())
        });
    let point = slots
        .get(frame_at)
        .and_then(Option::as_ref)
        .and_then(crate::tree::properties::node_attribute_decode::point);
    let size = slots
        .get(frame_at + 1)
        .and_then(Option::as_ref)
        .and_then(crate::tree::properties::node_attribute_decode::size);
    let frame = point
        .zip(size)
        .and_then(|(point, size)| crate::tree::element_bounds::rect_from_parts(point, size));
    Ok(ElementRead { role, frame })
}

fn slot(item: &core_foundation::base::CFType) -> Result<Option<core_foundation::base::CFType>, ()> {
    if crate::tree::properties::node_attribute_decode::is_null(item) {
        return Ok(None);
    }
    match crate::tree::properties::node_attribute_decode::slot_error(item) {
        Some(error) if crate::tree::properties::ax_absence::is_absent_attribute_error(error) => {
            Ok(None)
        }
        Some(_) => Err(()),
        None => Ok(Some(item.clone())),
    }
}
