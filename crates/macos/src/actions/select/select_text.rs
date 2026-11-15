//! Selects a run of text inside an editable text element by its content.
//!
//! The requested text must occur exactly once in the element's value, so the
//! selection never lands on a guess: no occurrence and several occurrences are
//! refused before anything is written. The range is written to
//! `AXSelectedTextRange` in UTF-16 code units, the unit AppKit text ranges use,
//! and the selection is read back until it equals the requested range. A
//! read-back that names a different range fails the action; only a selection
//! that cannot be read at all leaves the write delivered but unverified.

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

/// What `select_text` reads from and writes to a text element.
#[cfg(target_os = "macos")]
pub(crate) trait TextSelectionTarget {
    fn accepts_selection(&self, deadline: Deadline) -> Result<bool, AdapterError>;

    fn read_value(
        &self,
        deadline: Deadline,
    ) -> Result<Option<crate::tree::properties::bounded_string::BoundedString>, i32>;

    fn write_selection(&self, range: &Range<usize>, deadline: Deadline)
    -> Result<(), AdapterError>;

    fn read_selection(&self, deadline: Deadline) -> Option<Range<usize>>;
}

#[cfg(target_os = "macos")]
struct LiveTextElement<'a>(&'a crate::tree::AXElement);

#[cfg(target_os = "macos")]
impl TextSelectionTarget for LiveTextElement<'_> {
    fn accepts_selection(&self, deadline: Deadline) -> Result<bool, AdapterError> {
        crate::actions::ax_helpers::is_attr_settable(self.0, SELECTED_RANGE, deadline)
    }

    fn read_value(
        &self,
        deadline: Deadline,
    ) -> Result<Option<crate::tree::properties::bounded_string::BoundedString>, i32> {
        let mut usage = crate::tree::observation_usage::ObservationUsage::new(
            agent_desktop_core::ObservationBudget::default(),
        );
        crate::tree::properties::attributes::copy_value_typed_bounded_result(
            self.0, deadline, &mut usage,
        )
    }

    fn write_selection(
        &self,
        range: &Range<usize>,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        write_range(self.0, range, deadline)
    }

    fn read_selection(&self, deadline: Deadline) -> Option<Range<usize>> {
        crate::tree::properties::attributes::selected_text_range(self.0, deadline)
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn select_text(
    element: &crate::tree::AXElement,
    wanted: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    select_text_in(&LiveTextElement(element), wanted, deadline)
}

/// Selects `wanted` in `target`: refuses before writing when the value cannot
/// be read whole or the text does not occur exactly once, then writes the
/// range and reads the selection back.
#[cfg(target_os = "macos")]
pub(crate) fn select_text_in(
    target: &impl TextSelectionTarget,
    wanted: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if !target.accepts_selection(deadline)? {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "This text element does not accept a text selection",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    let read = target.read_value(deadline);
    if read.is_err() && deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_disposition(DeliverySemantics::not_delivered()));
    }
    let text = searchable_text(read)?;
    let range = unique_range(&text, wanted)?;
    target.write_selection(&range, deadline)?;
    verify_range(target, &range, deadline)
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
    target: &impl TextSelectionTarget,
    range: &Range<usize>,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let settle = std::time::Instant::now() + std::time::Duration::from_millis(300);
    loop {
        let selected = target.read_selection(deadline);
        if selected.as_ref() == Some(range) {
            return Ok(true);
        }
        if std::time::Instant::now() >= settle || deadline.is_expired() {
            return settled_selection(selected, range);
        }
        let pause = deadline.remaining_slice(std::time::Duration::from_millis(15))?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(15)));
    }
}

/// The element's text to search, or the refusal when it cannot be read whole.
#[cfg(target_os = "macos")]
pub(crate) fn searchable_text(
    read: Result<Option<crate::tree::properties::bounded_string::BoundedString>, i32>,
) -> Result<String, AdapterError> {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorInvalidUIElement,
    };

    match read {
        Ok(None) => Ok(String::new()),
        Ok(Some(text)) if text.complete => Ok(text.value),
        Ok(Some(_)) => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "The text element's value is too long to search for the text to select",
        )
        .with_details(serde_json::json!({ "kind": "text_value_truncated" }))
        .with_suggestion(
            "Select the text with the keyboard instead: click into the field, then move with arrow keys while holding shift.",
        )
        .with_disposition(DeliverySemantics::not_delivered())),
        Err(ax_error) => {
            let (code, suggestion) = if ax_error == kAXErrorAPIDisabled {
                (
                    ErrorCode::PermDenied,
                    "Grant Accessibility permission to the terminal running agent-desktop, then retry.",
                )
            } else if ax_error == kAXErrorInvalidUIElement {
                (
                    ErrorCode::StaleRef,
                    "Take a fresh snapshot and retry with the new element reference.",
                )
            } else if ax_error == kAXErrorCannotComplete {
                (
                    ErrorCode::AppUnresponsive,
                    "Wait until the application responds, then retry.",
                )
            } else {
                (
                    ErrorCode::ActionFailed,
                    "Take a fresh snapshot and retry with the new element reference.",
                )
            };
            Err(AdapterError::new(
                code,
                "Could not read the text element's value to find the text to select",
            )
            .with_details(serde_json::json!({
                "kind": "text_value_unreadable",
                "ax_error": ax_error,
            }))
            .with_suggestion(suggestion)
            .with_disposition(DeliverySemantics::not_delivered()))
        }
    }
}

/// What a selection read back after the settle window proves about the write.
pub(crate) fn settled_selection(
    observed: Option<Range<usize>>,
    requested: &Range<usize>,
) -> Result<bool, AdapterError> {
    match observed {
        None => Ok(false),
        Some(range) if range == *requested => Ok(true),
        Some(range) => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "The selection write completed but the element reports a different selection",
        )
        .with_details(serde_json::json!({
            "kind": "selection_not_observed",
            "requested": { "start": requested.start, "end": requested.end },
            "observed": { "start": range.start, "end": range.end },
        }))
        .with_suggestion(
            "The selection write was sent but not observed. Take a fresh snapshot, read the field's selection, and decide the next step from what it shows.",
        )
        .with_disposition(DeliverySemantics::delivered_unverified())),
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
