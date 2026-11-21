//! Role and frame reads for the scroll signals, one
//! `AXUIElementCopyMultipleAttributeValues` call per element instead of one
//! call per attribute. Only `kAXErrorAttributeUnsupported` and
//! `kAXErrorNoValue` make a frame absent; every other failure is unreadable,
//! so a transient error never reads as a frame that moved.

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
    decode_call(error, values.as_ref(), attributes.len() == 3)
}

fn decode_call(
    error: i32,
    values: Option<&core_foundation::array::CFArray<core_foundation::base::CFType>>,
    with_role: bool,
) -> Result<ElementRead, ()> {
    if error != accessibility_sys::kAXErrorSuccess {
        return is_absent_frame_error(error)
            .then(ElementRead::default)
            .ok_or(());
    }
    decode(values.ok_or(())?, with_role)
}

fn decode(
    values: &core_foundation::array::CFArray<core_foundation::base::CFType>,
    with_role: bool,
) -> Result<ElementRead, ()> {
    let slots = values.iter().map(|item| item.clone()).collect::<Vec<_>>();
    let frame_at = usize::from(with_role);
    if slots.len() != frame_at + 2 {
        return Err(());
    }
    let role = match slots.first() {
        Some(item) if with_role => role_slot(item)?,
        _ => None,
    };
    let position = slots.get(frame_at).ok_or(())?;
    let extent = slots.get(frame_at + 1).ok_or(())?;
    let frame = frame_slot(
        position,
        crate::tree::properties::node_attribute_decode::point,
    )?
    .zip(frame_slot(
        extent,
        crate::tree::properties::node_attribute_decode::size,
    )?)
    .and_then(|(point, size)| crate::tree::element_bounds::rect_from_parts(point, size));
    Ok(ElementRead { role, frame })
}

/// A role the element does not report is no role; any other failed read of
/// it is unreadable.
fn role_slot(item: &core_foundation::base::CFType) -> Result<Option<String>, ()> {
    use core_foundation::base::TCFType;

    if crate::tree::properties::node_attribute_decode::is_null(item) {
        return Ok(None);
    }
    match crate::tree::properties::node_attribute_decode::slot_error(item) {
        Some(error) if crate::tree::properties::ax_absence::is_absent_attribute_error(error) => {
            Ok(None)
        }
        Some(_) => Err(()),
        None => Ok(
            crate::tree::properties::cf_type::borrowed_cf_string(item.as_CFTypeRef())
                .map(|role| role.to_string()),
        ),
    }
}

/// A position or size the element reports as unsupported or without a value
/// is no frame. Any other error, a null slot or a value of another type is
/// unreadable, as the single-attribute read classified it.
fn frame_slot<T>(
    item: &core_foundation::base::CFType,
    decode: fn(&core_foundation::base::CFType) -> Option<T>,
) -> Result<Option<T>, ()> {
    use core_foundation::base::TCFType;

    if crate::tree::properties::node_attribute_decode::is_null(item) {
        return Err(());
    }
    if let Some(error) = crate::tree::properties::node_attribute_decode::slot_error(item) {
        return is_absent_frame_error(error).then_some(None).ok_or(());
    }
    let is_ax_value = unsafe {
        core_foundation_sys::base::CFGetTypeID(item.as_CFTypeRef())
            == accessibility_sys::AXValueGetTypeID()
    };
    if !is_ax_value {
        return Err(());
    }
    decode(item).map(Some).ok_or(())
}

fn is_absent_frame_error(error: i32) -> bool {
    error == accessibility_sys::kAXErrorAttributeUnsupported
        || error == accessibility_sys::kAXErrorNoValue
}

#[cfg(test)]
#[path = "scroll_frames_tests.rs"]
mod tests;
