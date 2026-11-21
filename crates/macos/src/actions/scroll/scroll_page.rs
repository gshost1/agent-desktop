use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode};
use std::time::Duration;

use super::scroll_signals::ScrollSignals;
use crate::tree::AXElement;

const SETTLE_READS: usize = 11;
const SETTLE_PAUSE: Duration = Duration::from_millis(25);
const SAMPLED_CHILDREN: usize = 4;
const AREA_CHILDREN: usize = 32;

/// Reads every signal a page scroll on `direction`'s axis could move: that
/// axis's scroll bar value, the frames of the area's first children that are
/// not scroll bars, and the frames of the first children of each of those,
/// so a pinned first child does not hide rows that moved beside it. A signal
/// that exists but cannot be read is recorded as unreadable, never as
/// unchanged. An element without a frame is no signal and is not counted.
pub(super) fn capture(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> ScrollSignals {
    let bar_attribute = match direction {
        Direction::Up | Direction::Down => "AXVerticalScrollBar",
        Direction::Left | Direction::Right => "AXHorizontalScrollBar",
    };
    let bar_value = match super::scroll_read::element(area, bar_attribute, deadline) {
        Ok(Some(bar)) => Some(
            super::scroll_read::number(&bar, "AXValue", deadline)
                .ok()
                .flatten(),
        ),
        Ok(None) => None,
        Err(_) => Some(None),
    };
    let at_end = area_at_end(bar_value, direction, || {
        content_fits(area, direction, deadline)
    });
    let mut sampled = 0;
    let content_children = content_children(area, deadline);
    let descendant = match &content_children {
        Some(Some(children)) => descendant_frames(children, &mut sampled, deadline),
        Some(None) => Some(None),
        None => None,
    };
    let content = content_children.map(|children| {
        children.and_then(|children| frames_hash(&children, &mut sampled, deadline))
    });
    ScrollSignals {
        bar: bar_value.map(|value| value.map(f64::to_bits)),
        content,
        descendant,
        sampled,
        at_end,
    }
}

/// Whether the area shows it cannot move toward `direction`: its scroll bar
/// on that axis reads a value at that end, or, when the axis has no scroll
/// bar, its content fits inside it on that axis. A bar whose value cannot be
/// read proves nothing.
pub(super) fn area_at_end(
    bar_value: Option<Option<f64>>,
    direction: &Direction,
    content_fits: impl FnOnce() -> bool,
) -> bool {
    match bar_value {
        Some(Some(value)) => {
            value.is_finite() && !super::scroll_plan::bar_can_move(value, direction)
        }
        Some(None) => false,
        None => content_fits(),
    }
}

fn content_fits(area: &AXElement, direction: &Direction, deadline: Deadline) -> bool {
    let Ok(instant) = crate::tree::locator_deadline::from_operation(deadline) else {
        return false;
    };
    let Some(frame) = crate::tree::element_bounds::read_bounds_with_deadline(area, instant)
        .ok()
        .flatten()
    else {
        return false;
    };
    let Some(clip) =
        crate::tree::element_bounds::clip_bounds(area, "AXScrollArea", Some(frame), instant)
    else {
        return false;
    };
    match direction {
        Direction::Up | Direction::Down => clip.height > frame.height,
        Direction::Left | Direction::Right => clip.width > frame.width,
    }
}

fn content_children(area: &AXElement, deadline: Deadline) -> Option<Option<Vec<AXElement>>> {
    let Ok(children) = super::scroll_read::children_up_to(area, AREA_CHILDREN, deadline) else {
        return Some(None);
    };
    let mut content = Vec::new();
    for child in children {
        match super::scroll_read::string(&child, "AXRole", deadline) {
            Ok(Some(role)) if role == "AXScrollBar" => continue,
            Ok(_) => content.push(child),
            Err(_) => return Some(None),
        }
        if content.len() == SAMPLED_CHILDREN {
            break;
        }
    }
    (!content.is_empty()).then_some(Some(content))
}

fn descendant_frames(
    children: &[AXElement],
    sampled: &mut usize,
    deadline: Deadline,
) -> Option<Option<u64>> {
    let mut inner = Vec::new();
    for child in children {
        match super::scroll_read::children_up_to(child, SAMPLED_CHILDREN, deadline) {
            Ok(grandchildren) => inner.extend(grandchildren),
            Err(_) => return Some(None),
        }
    }
    (!inner.is_empty()).then(|| frames_hash(&inner, sampled, deadline))
}

fn frames_hash(elements: &[AXElement], sampled: &mut usize, deadline: Deadline) -> Option<u64> {
    use std::hash::{Hash, Hasher};

    let mut hasher = rustc_hash::FxHasher::default();
    elements.len().hash(&mut hasher);
    let mut framed = 0;
    for element in elements {
        let frame = frame_hash(element, deadline)?;
        framed += usize::from(frame.is_some());
        frame.hash(&mut hasher);
    }
    *sampled += framed;
    Some(hasher.finish())
}

fn frame_hash(element: &AXElement, deadline: Deadline) -> Option<Option<u64>> {
    let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    let frame = crate::tree::element_bounds::read_bounds_with_deadline(element, instant).ok()?;
    match frame {
        Some(frame) => frame.bounds_hash().map(Some),
        None => Some(None),
    }
}

/// How long a refusal may take and still count as the application's answer.
/// `kAXErrorCannotComplete` also ends a call that ran into its messaging
/// timeout while the application was still working on it, which may scroll
/// after the settle window closes, so only an answer well inside that timeout
/// is settled. SwiftUI scroll views refuse a page they cannot move to in a
/// few milliseconds.
pub(super) fn answer_window(deadline: Deadline) -> Duration {
    crate::tree::properties::ax_ipc::mutation_timeout(deadline).unwrap_or(Duration::ZERO) / 2
}

pub(super) fn answered_in_time(elapsed: Duration, window: Duration) -> bool {
    elapsed < window
}

/// Settles a page action that failed on its first step. `kAXErrorCannotComplete`
/// alone cannot tell a hung application from a scroll area that has no page to
/// move to. Any signal that moved means the scroll happened. The action counts
/// as undelivered only when the area shows, before and after, that it is at
/// its end in that direction, and every signal it has stayed readable and
/// unchanged for the whole settle window. Unchanged signals alone are no
/// proof: they may miss what moved, and an animated scroll may start after
/// the window. Anything less keeps the original error and its uncertain
/// disposition.
pub(super) fn settle_refusal(
    area: &AXElement,
    direction: &Direction,
    before: ScrollSignals,
    error: AdapterError,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    settle_observed(
        super::page_action(direction),
        before,
        error,
        || capture(area, direction, deadline),
        || pause_for_settle(deadline),
    )
}

pub(super) fn settle_observed(
    action: &str,
    before: ScrollSignals,
    error: AdapterError,
    mut read_after: impl FnMut() -> ScrollSignals,
    mut pause: impl FnMut() -> bool,
) -> Result<(), AdapterError> {
    if !is_first_step_cannot_complete(&error) {
        return Err(error);
    }
    for read in 0..SETTLE_READS {
        let after = read_after();
        match before.movement(after) {
            Some(true) => return Ok(()),
            Some(false) if before.at_end && after.at_end => {}
            Some(false) | None => return Err(error),
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
            "{action} is not available: the scroll area refused it, it is already at its end \
             that way, and its content did not move"
        ),
    )
    .with_details(serde_json::json!({
        "kind": "scroll_action_unavailable",
        "operation": action,
        "ax_error": accessibility_sys::kAXErrorCannotComplete,
        "scroll_signals_unchanged": true,
        "scroll_area_at_end": true,
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
