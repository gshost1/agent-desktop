use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode};
use std::time::Duration;

use super::scroll_signals::ScrollSignals;
use crate::tree::AXElement;

const SETTLE_READS: usize = 11;
const SETTLE_PAUSE: Duration = Duration::from_millis(25);
const SAMPLED_CHILDREN: usize = 4;

/// Reads every signal a page scroll on `direction`'s axis could move: that
/// axis's scroll bar value, the frames of the area's first children that are
/// not scroll bars, and the frames of the first such child's first children.
/// A signal that exists but cannot be read is recorded as unreadable, never
/// as unchanged.
pub(super) fn capture(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> ScrollSignals {
    let bar_attribute = match direction {
        Direction::Up | Direction::Down => "AXVerticalScrollBar",
        Direction::Left | Direction::Right => "AXHorizontalScrollBar",
    };
    let bar = match super::scroll_read::element(area, bar_attribute, deadline) {
        Ok(Some(bar)) => Some(
            super::scroll_read::number(&bar, "AXValue", deadline)
                .ok()
                .flatten()
                .map(f64::to_bits),
        ),
        Ok(None) => None,
        Err(_) => Some(None),
    };
    let mut sampled = 0;
    let content_children = content_children(area, deadline);
    let descendant = match &content_children {
        Some(Some(children)) => children.first().and_then(
            |first| match super::scroll_read::children(first, deadline) {
                Ok(inner) if inner.is_empty() => None,
                Ok(inner) => Some(frames_hash(sample(&inner), &mut sampled, deadline)),
                Err(_) => Some(None),
            },
        ),
        Some(None) => Some(None),
        None => None,
    };
    let content = content_children.map(|children| {
        children.and_then(|children| frames_hash(&children, &mut sampled, deadline))
    });
    ScrollSignals {
        bar,
        content,
        descendant,
        sampled,
    }
}

fn content_children(area: &AXElement, deadline: Deadline) -> Option<Option<Vec<AXElement>>> {
    let Ok(children) = super::scroll_read::children(area, deadline) else {
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

fn sample(elements: &[AXElement]) -> &[AXElement] {
    &elements[..elements.len().min(SAMPLED_CHILDREN)]
}

fn frames_hash(elements: &[AXElement], sampled: &mut usize, deadline: Deadline) -> Option<u64> {
    use std::hash::{Hash, Hasher};

    let mut hasher = rustc_hash::FxHasher::default();
    elements.len().hash(&mut hasher);
    for element in elements {
        frame_hash(element, deadline)?.hash(&mut hasher);
    }
    *sampled += elements.len();
    Some(hasher.finish())
}

fn frame_hash(element: &AXElement, deadline: Deadline) -> Option<u64> {
    let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    crate::tree::element_bounds::read_bounds_with_deadline(element, instant)
        .ok()
        .flatten()
        .and_then(|frame| frame.bounds_hash())
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
/// as undelivered only when every signal the area has stayed readable and
/// unchanged for the whole settle window, long enough for an animated scroll
/// to start. Anything less keeps the original error and its uncertain
/// disposition, because an unchanged or unreadable state is not proof.
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
        match before.movement(read_after()) {
            Some(true) => return Ok(()),
            Some(false) => {}
            None => return Err(error),
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
        "scroll_signals_unchanged": true,
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
