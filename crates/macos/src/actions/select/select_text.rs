//! Selects a run of text inside an editable text element by its content.
//!
//! The requested text must occur exactly once in the element's value, so the
//! selection never lands on a guess: no occurrence and several occurrences are
//! refused before anything is written. The range is written to
//! `AXSelectedTextRange` in UTF-16 code units, the unit AppKit text ranges use,
//! and the selection is read back until it equals the requested range.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use std::ops::Range;

const SELECTED_RANGE: &str = "AXSelectedTextRange";
const REPORTED_OFFSETS: usize = 10;

/// Where `wanted` occurs in `text`, as UTF-16 ranges.
pub(crate) fn utf16_occurrences(text: &str, wanted: &str) -> Vec<Range<usize>> {
    if wanted.is_empty() {
        return Vec::new();
    }
    let length = wanted.encode_utf16().count();
    text.match_indices(wanted)
        .map(|(byte, _)| {
            let start = text[..byte].encode_utf16().count();
            start..start + length
        })
        .collect()
}

/// The single range to select, or the refusal that explains why none is.
pub(crate) fn unique_range(text: &str, wanted: &str) -> Result<Range<usize>, AdapterError> {
    if wanted.is_empty() {
        return Err(
            AdapterError::new(ErrorCode::InvalidArgs, "Text to select must not be empty")
                .with_disposition(DeliverySemantics::not_delivered()),
        );
    }
    let mut found = utf16_occurrences(text, wanted);
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(AdapterError::new(
            ErrorCode::ElementNotFound,
            "The text element does not contain the text to select",
        )
        .with_details(serde_json::json!({ "kind": "text_not_found" }))
        .with_suggestion(
            "Read the element's value with 'get <ref> --property value' and pass text it contains exactly, including case.",
        )
        .with_disposition(DeliverySemantics::not_delivered())),
        count => Err(AdapterError::new(
            ErrorCode::AmbiguousTarget,
            format!("The text to select occurs {count} times in the element"),
        )
        .with_details(serde_json::json!({
            "kind": "text_occurs_more_than_once",
            "occurrences": count,
            "offsets": found
                .iter()
                .take(REPORTED_OFFSETS)
                .map(|range| range.start)
                .collect::<Vec<_>>(),
        }))
        .with_suggestion(
            "Add neighbouring words so the text occurs once, then move within the selection with arrow keys if only part of it is needed.",
        )
        .with_disposition(DeliverySemantics::not_delivered())),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn select_text(
    element: &crate::tree::AXElement,
    wanted: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if !crate::actions::ax_helpers::is_attr_settable(element, SELECTED_RANGE, deadline)? {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "This text element does not accept a text selection",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    let text = crate::tree::copy_value_typed(element, deadline).unwrap_or_default();
    let range = unique_range(&text, wanted)?;
    write_range(element, &range, deadline)?;
    verify_range(element, &range, deadline)
}

#[cfg(target_os = "macos")]
fn write_range(
    element: &crate::tree::AXElement,
    range: &Range<usize>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use accessibility_sys::{AXValueCreate, kAXValueTypeCFRange};
    use core_foundation::{
        base::{CFType, TCFType},
        string::CFString,
    };
    use core_foundation_sys::base::CFRange;

    let overflow = || AdapterError::new(ErrorCode::InvalidArgs, "Text range is out of range");
    let raw_range = CFRange {
        location: isize::try_from(range.start).map_err(|_| overflow())?,
        length: isize::try_from(range.len()).map_err(|_| overflow())?,
    };
    let value =
        unsafe { AXValueCreate(kAXValueTypeCFRange, (&raw_range as *const CFRange).cast()) };
    if value.is_null() {
        return Err(AdapterError::internal(
            "AXValueCreate returned null for a text range",
        ));
    }
    let value = unsafe { CFType::wrap_under_create_rule(value as _) };
    let attribute = CFString::new(SELECTED_RANGE);
    let error = crate::tree::properties::ax_ipc::set_attribute_value(
        element,
        attribute.as_concrete_TypeRef(),
        value.as_CFTypeRef(),
        deadline,
    )?;
    let delivered = crate::actions::ax_mutation::classify_result(
        SELECTED_RANGE,
        "AXUIElementSetAttributeValue",
        error,
    )?;
    if delivered {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "The text element refused the selection write",
    )
    .with_disposition(DeliverySemantics::not_delivered()))
}

#[cfg(target_os = "macos")]
fn verify_range(
    element: &crate::tree::AXElement,
    range: &Range<usize>,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let settle = std::time::Instant::now() + std::time::Duration::from_millis(300);
    loop {
        let selected = crate::tree::properties::attributes::selected_text_range(element, deadline);
        if selected.as_ref() == Some(range) {
            return Ok(true);
        }
        if std::time::Instant::now() >= settle || deadline.is_expired() {
            return Ok(false);
        }
        let pause = deadline.remaining_slice(std::time::Duration::from_millis(15))?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(15)));
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn select_text(
    _element: &crate::tree::AXElement,
    _wanted: &str,
    _deadline: Deadline,
) -> Result<bool, AdapterError> {
    Err(AdapterError::not_supported("select_text"))
}

#[cfg(test)]
#[path = "select_text_tests.rs"]
mod tests;
