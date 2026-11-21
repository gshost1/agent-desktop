//! The window attributes the inventory joins to CoreGraphics records, read in
//! one `AXUIElementCopyMultipleAttributeValues` call per AX window.
//!
//! A title the application cannot report leaves the title unknown and never
//! fails the inventory, because the inventory read no title before titles were
//! joined from accessibility. A minimized state that fails for a transport
//! reason still fails, as the single-attribute read did.

use agent_desktop_core::{AdapterError, Deadline, Rect};
use std::time::Instant;

const TITLE: &str = "AXTitle";
const MINIMIZED: &str = "AXMinimized";

pub(super) fn read_geometry_baseline(
    window: &crate::tree::AXElement,
    deadline: Deadline,
) -> Option<Rect> {
    use accessibility_sys::{AXValueGetTypeID, kAXErrorSuccess};
    use core_foundation::{array::CFArray, base::TCFType, string::CFString};

    let position = CFString::new("AXPosition");
    let size = CFString::new("AXSize");
    let requested =
        CFArray::from_copyable(&[position.as_concrete_TypeRef(), size.as_concrete_TypeRef()]);
    let (error, values) = crate::tree::properties::ax_ipc::copy_multiple_attribute_values(
        window,
        requested.as_concrete_TypeRef(),
        deadline,
    );
    let values = crate::tree::properties::cf_type::created_cf_array(values)?;
    if error != kAXErrorSuccess || deadline.is_expired() || values.len() != 2 {
        return None;
    }
    let position = values.get(0)?;
    let size = values.get(1)?;
    let ax_value_type = unsafe { AXValueGetTypeID() };
    if position.type_of() != ax_value_type || size.type_of() != ax_value_type {
        return None;
    }
    use crate::tree::properties::node_attribute_decode;
    crate::tree::element_bounds::rect_from_parts(
        node_attribute_decode::point(&position)?,
        node_attribute_decode::size(&size)?,
    )
}

pub(crate) fn read_title_and_minimized(
    window: &crate::tree::AXElement,
    deadline: Instant,
) -> Result<(Option<String>, Option<bool>), AdapterError> {
    use core_foundation::{array::CFArray, base::TCFType, string::CFString};

    crate::tree::surface_read::ensure_before_deadline(deadline)?;
    let names = [CFString::new(TITLE), CFString::new(MINIMIZED)];
    let refs = names
        .iter()
        .map(|name| name.as_concrete_TypeRef())
        .collect::<Vec<_>>();
    let requested = CFArray::from_copyable(&refs);
    let (error, values) = crate::tree::properties::ax_ipc::copy_multiple_attribute_values(
        window,
        requested.as_concrete_TypeRef(),
        deadline,
    );
    let values = crate::tree::properties::cf_type::created_cf_array(values);
    if error != accessibility_sys::kAXErrorSuccess {
        if crate::tree::properties::ax_absence::is_absent_attribute_error(error) {
            return Ok((None, None));
        }
        return Err(crate::tree::surface_read::map_error(MINIMIZED, error));
    }
    crate::tree::surface_read::ensure_before_deadline(deadline)?;
    let Some(values) = values else {
        return Ok((None, None));
    };
    decode(&values)
}

fn decode(
    values: &core_foundation::array::CFArray<core_foundation::base::CFType>,
) -> Result<(Option<String>, Option<bool>), AdapterError> {
    use core_foundation::{boolean::CFBoolean, string::CFString};

    let mut title = None;
    let mut minimized = None;
    for (index, item) in values.iter().enumerate().take(2) {
        if crate::tree::properties::node_attribute_decode::is_null(&item) {
            continue;
        }
        if let Some(error) = crate::tree::properties::node_attribute_decode::slot_error(&item) {
            if index == 1 && !crate::tree::properties::ax_absence::is_absent_attribute_error(error)
            {
                return Err(crate::tree::surface_read::map_error(MINIMIZED, error));
            }
            continue;
        }
        if index == 0 {
            title = item.downcast::<CFString>().map(|value| value.to_string());
        } else {
            minimized = item.downcast::<CFBoolean>().map(bool::from);
        }
    }
    Ok((title, minimized))
}

#[cfg(test)]
#[path = "window_ax_attributes_tests.rs"]
mod tests;
