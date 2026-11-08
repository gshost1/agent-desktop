//! Reads the per-window accessibility fields that window inventory joins onto
//! CoreGraphics records, in one batched IPC per window.

use agent_desktop_core::AdapterError;
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    string::{CFString, CFStringRef},
};
use std::time::Instant;

const MINIMIZED: &str = "AXMinimized";
const TITLE: &str = "AXTitle";
const MALFORMED_AX_VALUE: i32 = i32::MIN;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WindowAxFields {
    pub(crate) minimized: Option<bool>,
    pub(crate) title: Option<String>,
}

/// `AXMinimized` keeps the strict semantics of a single read: a transport
/// failure fails the window. `AXTitle` is best effort, so any failure on its
/// slot reads as no title.
pub(crate) fn read_window_fields(
    window: &crate::tree::AXElement,
    deadline: Instant,
) -> Result<WindowAxFields, AdapterError> {
    use accessibility_sys::{kAXErrorAPIDisabled, kAXErrorSuccess};

    crate::tree::surface_read::ensure_before_deadline(deadline)?;
    let names = [CFString::new(MINIMIZED), CFString::new(TITLE)];
    let refs = names
        .iter()
        .map(TCFType::as_concrete_TypeRef)
        .collect::<Vec<CFStringRef>>();
    let attributes = CFArray::from_copyable(&refs);
    let (error, value) = crate::tree::properties::ax_ipc::copy_multiple_attribute_values(
        window,
        attributes.as_concrete_TypeRef(),
        deadline,
    );
    let items = crate::tree::properties::cf_type::created_cf_array(value);
    if error == kAXErrorAPIDisabled {
        return Err(crate::tree::surface_read::map_error(MINIMIZED, error));
    }
    crate::tree::surface_read::ensure_before_deadline(deadline)?;
    match items {
        Some(items) if error == kAXErrorSuccess => decode_window_fields(&items)
            .map_err(|error| crate::tree::surface_read::map_error(MINIMIZED, error)),
        _ if error == kAXErrorSuccess
            || crate::tree::properties::ax_absence::is_absent_attribute_error(error) =>
        {
            read_window_fields_separately(window, deadline)
        }
        _ => Err(crate::tree::surface_read::map_error(MINIMIZED, error)),
    }
}

fn read_window_fields_separately(
    window: &crate::tree::AXElement,
    deadline: Instant,
) -> Result<WindowAxFields, AdapterError> {
    let minimized = crate::tree::surface_read::boolean(window, MINIMIZED, deadline)?;
    let title = crate::tree::surface_read::string(window, TITLE, deadline)
        .ok()
        .flatten()
        .filter(|title| !title.is_empty());
    Ok(WindowAxFields { minimized, title })
}

fn decode_window_fields(items: &CFArray<CFType>) -> Result<WindowAxFields, i32> {
    if items.len() != 2 {
        return Err(MALFORMED_AX_VALUE);
    }
    let (Some(minimized), Some(title)) = (items.get(0), items.get(1)) else {
        return Err(MALFORMED_AX_VALUE);
    };
    Ok(WindowAxFields {
        minimized: decode_minimized(&minimized)?,
        title: decode_title(&title),
    })
}

fn decode_minimized(item: &CFType) -> Result<Option<bool>, i32> {
    if let Some(error) = crate::tree::properties::node_attribute_decode::slot_error(item) {
        return if crate::tree::properties::ax_absence::is_absent_attribute_error(error) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    if crate::tree::properties::node_attribute_decode::is_null(item) {
        return Ok(None);
    }
    item.downcast::<CFBoolean>()
        .map(|value| Some(bool::from(value)))
        .ok_or(MALFORMED_AX_VALUE)
}

fn decode_title(item: &CFType) -> Option<String> {
    item.downcast::<CFString>()
        .map(|title| title.to_string())
        .filter(|title| !title.is_empty())
}

#[cfg(test)]
#[path = "window_ax_fields_tests.rs"]
mod tests;
