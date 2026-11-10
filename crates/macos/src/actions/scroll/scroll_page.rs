use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Rect};
use std::time::Duration;

use crate::tree::AXElement;

const SETTLE_READS: usize = 3;
const SETTLE_PAUSE: Duration = Duration::from_millis(20);

/// Reads where a scroll area's content sits: the frame of its first child that
/// is not a scroll bar. Scrolling moves that frame, so comparing it before and
/// after a page action shows whether the action moved anything. A failed read
/// is no observation, never a claim that nothing moved.
pub(super) fn content_frame(area: &AXElement, deadline: Deadline) -> Option<Rect> {
    let children = super::scroll_read::children(area, deadline).ok()?;
    for child in children {
        let role = super::scroll_read::string(&child, "AXRole", deadline).ok()?;
        if role.as_deref() == Some("AXScrollBar") {
            continue;
        }
        let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
        return crate::tree::element_bounds::read_bounds_with_deadline(&child, instant)
            .ok()
            .flatten();
    }
    None
}

/// Settles a page action that failed on its first step. `kAXErrorCannotComplete`
/// alone cannot tell a hung application from a scroll area that has no page
/// to move to, so the content frame read before the action is compared with a
/// fresh read: content that stayed put proves the action did nothing, which is
/// an unavailable action and not an unresponsive application. Content that
/// moved means the scroll happened. Any other error, or a frame that cannot be
/// read, keeps the original error and its uncertain disposition.
pub(super) fn settle_refusal(
    area: &AXElement,
    action: &str,
    before: Option<Rect>,
    error: AdapterError,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    settle_observed(
        action,
        before,
        error,
        || content_frame(area, deadline),
        || pause_for_settle(deadline),
    )
}

pub(super) fn settle_observed(
    action: &str,
    before: Option<Rect>,
    error: AdapterError,
    mut read_after: impl FnMut() -> Option<Rect>,
    mut pause: impl FnMut() -> bool,
) -> Result<(), AdapterError> {
    if !is_first_step_cannot_complete(&error) {
        return Err(error);
    }
    let Some(before) = before.and_then(|frame| frame.bounds_hash()) else {
        return Err(error);
    };
    for read in 0..SETTLE_READS {
        let Some(after) = read_after() else {
            return Err(error);
        };
        if after.bounds_hash() != Some(before) {
            return Ok(());
        }
        if read + 1 < SETTLE_READS && !pause() {
            return Err(error);
        }
    }
    Err(unavailable_page_action(action, &error))
}

fn pause_for_settle(deadline: Deadline) -> bool {
    let Ok(pause) = deadline.remaining_slice(SETTLE_PAUSE) else {
        return false;
    };
    std::thread::sleep(pause.min(SETTLE_PAUSE));
    true
}

fn is_first_step_cannot_complete(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("ax_error"))
        .and_then(serde_json::Value::as_i64)
        == Some(i64::from(accessibility_sys::kAXErrorCannotComplete))
}

fn unavailable_page_action(action: &str, source: &AdapterError) -> AdapterError {
    let error = AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!(
            "{action} is not available: the scroll area refused it and its content did not move"
        ),
    )
    .with_details(serde_json::json!({
        "kind": "scroll_action_unavailable",
        "operation": action,
        "ax_error": accessibility_sys::kAXErrorCannotComplete,
        "content_moved": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
    .with_suggestion(
        "The scroll area cannot move further this way. Scroll in the other direction, \
         or target an element inside a different scroll area.",
    );
    match source.platform_detail.as_deref() {
        Some(detail) => error.with_platform_detail(detail),
        None => error,
    }
}

#[cfg(test)]
#[path = "scroll_page_tests.rs"]
mod tests;
