//! One page of a scroll area for a search whose target has not loaded yet.
//!
//! A search must not move further than the loaded content in one step, or a
//! lazily loaded list skips rows it never builds. The first step brings the
//! farthest loaded item into view: it stays loaded, so the next observation
//! overlaps this one, and a list that loaded everything jumps to its end
//! without skipping anything unseen. When that moves nothing, the step falls
//! to page-sized steps: the scroll bar's page part, the area's page action,
//! a bar value write measured to the viewport, then the bar's line
//! increment. It never sets the bar's value by a fixed fraction, which on a
//! long list jumps many viewports at once (a SwiftUI lazy stack moved 40
//! rows past an 18-row load on one shift of a tenth of its range). A reveal
//! reads movement from the target's bounds; a search has no target yet, so
//! it reads the area's own scroll signals.

use agent_desktop_core::{AdapterError, Deadline, Direction, ErrorCode, ScrollPage};
use std::time::Duration;

use super::scroll_signals::ScrollSignals;
use crate::tree::AXElement;

const SETTLE_READS: usize = 8;
const SETTLE_PAUSE: Duration = Duration::from_millis(25);
const MAX_WRAPPER_DEPTH: usize = 3;

pub(crate) fn page_scroll_area(
    element: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<ScrollPage, AdapterError> {
    let area = super::find_scroll_area(element, deadline)?.unwrap_or_else(|| element.clone());
    let room = bar_room(&area, direction, deadline)?;
    if room == Some(false) {
        return Ok(ScrollPage::AtEnd { delivered: false });
    }
    let before = super::scroll_page::capture(&area, direction, deadline);
    let mut farthest = || reveal_farthest_item(&area, direction, deadline);
    let mut page_part = || press_bar_part(&area, direction, deadline);
    let mut page_action = || super::try_action(&area, super::page_action(direction), deadline);
    let mut value_step =
        || super::scroll_search_value::calibrated_value_step(&area, direction, deadline);
    let mut line_step = || bar_line_step(&area, direction, deadline);
    send_steps(
        &mut [
            &mut farthest,
            &mut page_part,
            &mut page_action,
            &mut value_step,
            &mut line_step,
        ],
        room == Some(true),
        || {
            settle(
                before,
                || super::scroll_page::capture(&area, direction, deadline),
                || pause(deadline),
            )
        },
    )
}

/// Sends steps in order until one moves the area, and settles what it did.
/// A step refused before delivery, or refused with the area proven unmoved,
/// passes to the next step, and so does a delivered step that moved nothing
/// while the scroll bar still has room. A refusal whose effect the reads
/// cannot decide ends the page with that error: something may have been sent.
pub(super) fn send_steps(
    steps: &mut [&mut dyn FnMut() -> Result<bool, AdapterError>],
    room_left: bool,
    mut settled: impl FnMut() -> ScrollPage,
) -> Result<ScrollPage, AdapterError> {
    let mut refusal = None;
    for step in steps.iter_mut() {
        match step() {
            Ok(false) => {}
            Ok(true) => match settled() {
                ScrollPage::AtEnd { .. } if room_left => refusal = Some(inert_area()),
                page => return Ok(page),
            },
            Err(error) if super::scroll_into_view::declined_without_delivery(&error) => {
                refusal = Some(error);
            }
            Err(error) => match settled() {
                ScrollPage::Moved => return Ok(ScrollPage::Moved),
                ScrollPage::AtEnd { .. } => refusal = Some(error),
                ScrollPage::Unconfirmed => return Err(error),
            },
        }
    }
    Err(refusal.unwrap_or_else(no_page_step))
}

/// `Some(true)` when the bar on `direction`'s axis can still move that way,
/// `Some(false)` at its limit, `None` when the area has no readable bar.
fn bar_room(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<Option<bool>, AdapterError> {
    let (bar_attribute, _) = super::scroll_bar_action(direction);
    let Some(bar) = super::scroll_read::element(area, bar_attribute, deadline)? else {
        return Ok(None);
    };
    Ok(super::scroll_read::number(&bar, "AXValue", deadline)?
        .map(|value| super::scroll_plan::bar_can_move(value, direction)))
}

fn reveal_farthest_item(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let items = loaded_items(area, deadline)?;
    let farthest = match direction {
        Direction::Down | Direction::Right => items.last(),
        Direction::Up | Direction::Left => items.first(),
    };
    match farthest {
        Some(item) => super::try_action(item, "AXScrollToVisible", deadline),
        None => Ok(false),
    }
}

/// The items the area has loaded: its content's children, below any chain
/// of single-child wrappers.
pub(super) fn loaded_items(
    area: &AXElement,
    deadline: Deadline,
) -> Result<Vec<AXElement>, AdapterError> {
    let mut items = content_of(area, deadline)?;
    for _ in 0..MAX_WRAPPER_DEPTH {
        let [only] = items.as_slice() else { break };
        items = super::scroll_read::children(only, deadline)?;
    }
    Ok(items)
}

fn content_of(area: &AXElement, deadline: Deadline) -> Result<Vec<AXElement>, AdapterError> {
    let mut content = Vec::new();
    for child in super::scroll_read::children(area, deadline)? {
        if super::scroll_read::string(&child, "AXRole", deadline)?.as_deref() != Some("AXScrollBar")
        {
            content.push(child);
        }
    }
    Ok(content)
}

fn press_bar_part(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let (bar_attribute, _) = super::scroll_bar_action(direction);
    let Some(bar) = super::scroll_read::element(area, bar_attribute, deadline)? else {
        return Ok(false);
    };
    let part = match direction {
        Direction::Down | Direction::Right => "AXIncrementPage",
        Direction::Up | Direction::Left => "AXDecrementPage",
    };
    for child in super::scroll_read::children(&bar, deadline)? {
        if super::scroll_read::string(&child, "AXSubrole", deadline)?.as_deref() == Some(part) {
            return super::try_action(&child, "AXPress", deadline);
        }
    }
    Ok(false)
}

fn bar_line_step(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let (bar_attribute, increment) = super::scroll_bar_action(direction);
    match super::scroll_read::element(area, bar_attribute, deadline)? {
        Some(bar) => super::try_action(&bar, increment, deadline),
        None => Ok(false),
    }
}

fn no_page_step() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The scroll area offers no step a search can take without skipping rows",
    )
    .with_details(serde_json::json!({ "kind": "scroll_search_no_page_step" }))
    .with_suggestion(
        "Scroll the area with 'scroll' and read it after each step, or search a filter field if the app has one.",
    )
}

fn inert_area() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The scroll area did not move although its scroll bar has room",
    )
    .with_details(serde_json::json!({ "kind": "scroll_search_inert" }))
    .with_suggestion(
        "Scroll the area with 'scroll' and read it after each step, or search a filter field if the app has one.",
    )
}

/// Whether the area itself moved. A readable scroll bar decides alone: the
/// frames move too when an enclosing view scrolls, which is not this area
/// paging.
pub(super) fn area_moved(before: ScrollSignals, after: ScrollSignals) -> Option<bool> {
    match (before.bar, after.bar) {
        (Some(Some(was)), Some(Some(now))) => Some(was != now),
        _ => before.movement(after),
    }
}

/// Reads the area until it moves, which settles the page as moved. An area
/// that stays readable and unmoved through the last read settles as the end
/// of the area. Anything the reads cannot decide is unconfirmed.
pub(super) fn settle(
    before: ScrollSignals,
    mut read: impl FnMut() -> ScrollSignals,
    mut pause: impl FnMut() -> bool,
) -> ScrollPage {
    let mut unmoved = false;
    for attempt in 0..SETTLE_READS {
        match area_moved(before, read()) {
            Some(true) => return ScrollPage::Moved,
            Some(false) => unmoved = true,
            None => unmoved = false,
        }
        if attempt + 1 < SETTLE_READS && !pause() {
            break;
        }
    }
    if unmoved {
        ScrollPage::AtEnd { delivered: true }
    } else {
        ScrollPage::Unconfirmed
    }
}

fn pause(deadline: Deadline) -> bool {
    let Ok(slice) = deadline.remaining_slice(SETTLE_PAUSE) else {
        return false;
    };
    std::thread::sleep(slice.min(SETTLE_PAUSE));
    true
}

#[cfg(test)]
#[path = "scroll_search_tests.rs"]
mod tests;
