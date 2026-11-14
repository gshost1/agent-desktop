use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use foreign_types::ForeignType;
use std::time::Duration;

const KEY_SETTLE: Duration = Duration::from_millis(4);
const TEXT_CHUNK_BUDGET: Duration = Duration::from_millis(400);
const MAX_TEXT_UTF16: usize = 1_000_000;
const TEXT_CHUNK_UTF16: usize = 32;

pub(crate) fn post_key(
    key_code: u16,
    flags: CGEventFlags,
    target_pid: Option<i32>,
    deadline: Deadline,
    progress: (usize, usize),
) -> Result<(), AdapterError> {
    let mut delivery = crate::actions::DeliveryTracker::from_delivered_units(progress.0);
    ensure_budget(deadline, progress.1, delivery)?;
    let down = create_key_event(key_code, true, flags).map_err(|error| delivery.annotate(error))?;
    let up = create_key_event(key_code, false, flags).map_err(|error| delivery.annotate(error))?;
    post_pair((down, up), target_pid, deadline, &mut delivery, progress.1)
}

pub(crate) fn post_text(
    text: &str,
    target_pid: i32,
    deadline: Deadline,
    mut verify_target: impl FnMut(Deadline) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    preflight_text(text, deadline)?;
    let chunks = text_chunks(text)?;
    let total = chunks.len();
    let mut delivery = crate::actions::DeliveryTracker::default();
    for chunk in chunks {
        ensure_budget(deadline, total, delivery)?;
        verify_target(deadline).map_err(|error| {
            delivery.annotate(error.with_details(serde_json::json!({
                "delivered_chunks": delivery.delivered_units(),
                "total_chunks": total,
            })))
        })?;
        let down = create_key_event(0, true, CGEventFlags::empty())
            .map_err(|error| delivery.annotate(error))?;
        down.set_string_from_utf16_unchecked(&chunk);
        let up = create_key_event(0, false, CGEventFlags::empty())
            .map_err(|error| delivery.annotate(error))?;
        post_pair((down, up), Some(target_pid), deadline, &mut delivery, total)?;
    }
    Ok(())
}

pub(crate) fn preflight_text(text: &str, deadline: Deadline) -> Result<(), AdapterError> {
    let chunks = planned_chunk_count(text)?;
    let multiplier = u32::try_from(chunks)
        .map_err(|_| AdapterError::new(ErrorCode::InvalidArgs, "Text payload is too large"))?;
    let required = TEXT_CHUNK_BUDGET
        .checked_mul(multiplier)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Text payload is too large"))?;
    let remaining = deadline.remaining();
    if remaining < required {
        return Err(crate::actions::DeliveryTracker::default().annotate(
            AdapterError::timeout("Text cannot be delivered safely within the remaining deadline")
                .with_details(serde_json::json!({
                    "delivered_chunks": 0,
                    "total_chunks": chunks,
                    "required_ms": required.as_millis(),
                    "remaining_ms": remaining.as_millis(),
                })),
        ));
    }
    Ok(())
}

fn post_pair(
    events: (CGEvent, CGEvent),
    target_pid: Option<i32>,
    deadline: Deadline,
    delivery: &mut crate::actions::DeliveryTracker,
    total: usize,
) -> Result<(), AdapterError> {
    ensure_budget(deadline, total, *delivery)?;
    let (down, up) = events;
    let mut release = KeyReleaseGuard {
        event: Some(up),
        target_pid,
    };
    post(&down, target_pid);
    delivery.mark_delivered();
    sleep_bounded(deadline, KEY_SETTLE, *delivery, total)?;
    let up = release
        .event
        .take()
        .ok_or_else(|| AdapterError::internal("Keyboard release guard lost its key-up event"))
        .map_err(|error| delivery.annotate(error))?;
    post(&up, target_pid);
    ensure_budget(deadline, total, *delivery)
}

struct KeyReleaseGuard {
    event: Option<CGEvent>,
    target_pid: Option<i32>,
}

impl Drop for KeyReleaseGuard {
    fn drop(&mut self) {
        if let Some(event) = self.event.take() {
            post(&event, self.target_pid);
        }
    }
}

fn create_key_event(
    key_code: CGKeyCode,
    down: bool,
    flags: CGEventFlags,
) -> Result<CGEvent, AdapterError> {
    let event = unattributed_key_event(key_code, down)?;
    event.set_flags(flags);
    Ok(event)
}

/// Builds and drops one key event. The first key event a process builds on its main thread
/// loads the keyboard layout its characters come from, 9-10 ms. An event built on another
/// thread carries no characters, so this runs on the calling thread.
pub(crate) fn prepare_key_events() -> Result<(), AdapterError> {
    unattributed_key_event(0, true).map(drop)
}

/// Key events carry no event source, as a key event an application builds
/// for itself does. An event built from the HID system state source differs
/// from it only in being stamped as hardware input (source state 1), and a
/// Return built without a source is the one observed to commit an in-place
/// edit whose text came from an accessibility write.
fn unattributed_key_event(key_code: CGKeyCode, down: bool) -> Result<CGEvent, AdapterError> {
    let raw = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), key_code, down) };
    if raw.is_null() {
        return Err(AdapterError::internal("CGEventCreateKeyboardEvent failed"));
    }
    Ok(unsafe { CGEvent::from_ptr(raw) })
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: *const std::ffi::c_void,
        virtual_key: CGKeyCode,
        key_down: bool,
    ) -> *mut <CGEvent as ForeignType>::CType;
}

fn post(event: &CGEvent, target_pid: Option<i32>) {
    if let Some(pid) = target_pid {
        event.post_to_pid(pid);
    } else {
        event.post(CGEventTapLocation::HID);
    }
}

fn sleep_bounded(
    deadline: Deadline,
    duration: Duration,
    delivery: crate::actions::DeliveryTracker,
    total: usize,
) -> Result<(), AdapterError> {
    let pause = deadline
        .remaining_slice(duration)
        .map_err(|error| delivery.annotate(error))?;
    std::thread::sleep(pause);
    if pause < duration {
        ensure_budget(deadline, total, delivery)
    } else {
        Ok(())
    }
}

fn ensure_budget(
    deadline: Deadline,
    total: usize,
    delivery: crate::actions::DeliveryTracker,
) -> Result<(), AdapterError> {
    if !deadline.is_expired() {
        return Ok(());
    }
    Err(
        delivery.annotate(deadline.timeout_error().with_details(serde_json::json!({
            "delivered_chunks": delivery.delivered_units(),
            "total_chunks": total,
        }))),
    )
}

fn text_chunks(text: &str) -> Result<Vec<Vec<u16>>, AdapterError> {
    let _ = planned_chunk_count(text)?;
    let mut chunks = Vec::new();
    let mut current = Vec::with_capacity(TEXT_CHUNK_UTF16);
    for character in text.chars() {
        let mut encoded = [0_u16; 2];
        let encoded = character.encode_utf16(&mut encoded);
        if !current.is_empty() && current.len() + encoded.len() > TEXT_CHUNK_UTF16 {
            chunks.push(std::mem::take(&mut current));
        }
        current.extend_from_slice(encoded);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    Ok(chunks)
}

fn planned_chunk_count(text: &str) -> Result<usize, AdapterError> {
    let mut units = 0_usize;
    let mut chunks = 0_usize;
    let mut current = 0_usize;
    for character in text.chars() {
        let encoded = character.len_utf16();
        units = units.checked_add(encoded).ok_or_else(text_too_large)?;
        if units > MAX_TEXT_UTF16 {
            return Err(text_too_large());
        }
        if current > 0 && current + encoded > TEXT_CHUNK_UTF16 {
            chunks += 1;
            current = 0;
        }
        current += encoded;
    }
    Ok(chunks + usize::from(current > 0))
}

fn text_too_large() -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        "Text input must not exceed 1000000 UTF-16 code units",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_chunks_preserve_unicode_without_splitting_surrogates() {
        let text = format!("{}😀tail", "a".repeat(TEXT_CHUNK_UTF16 - 1));
        let chunks = text_chunks(&text).unwrap();
        let roundtrip = String::from_utf16(&chunks.concat()).unwrap();

        assert_eq!(roundtrip, text);
        assert!(chunks.iter().all(|chunk| chunk.len() <= TEXT_CHUNK_UTF16));
    }

    #[test]
    fn text_budget_rejects_unbounded_payloads() {
        let text = "x".repeat(MAX_TEXT_UTF16 + 1);
        let error = text_chunks(&text).expect_err("oversized text must fail before CGEvent");

        assert_eq!(error.code, ErrorCode::InvalidArgs);
    }

    #[test]
    fn key_and_text_events_are_not_stamped_as_hardware_input() {
        use core_graphics::event::EventField;

        for event in [
            create_key_event(36, true, CGEventFlags::empty()).unwrap(),
            create_key_event(36, false, CGEventFlags::CGEventFlagCommand).unwrap(),
            create_key_event(0, true, CGEventFlags::empty()).unwrap(),
        ] {
            assert_eq!(
                event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID),
                0
            );
        }
        let command = create_key_event(36, true, CGEventFlags::CGEventFlagCommand).unwrap();
        assert_eq!(command.get_flags(), CGEventFlags::CGEventFlagCommand);
        assert_eq!(
            command.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
            36
        );
    }

    #[test]
    fn impossible_delivery_deadline_fails_before_input() {
        let deadline = Deadline::after(1).unwrap();
        let error = preflight_text("payload", deadline).expect_err("deadline must reject plan");

        assert_eq!(error.code, ErrorCode::Timeout);
        assert_eq!(
            error.disposition,
            agent_desktop_core::DeliverySemantics::not_delivered()
        );
    }
}
