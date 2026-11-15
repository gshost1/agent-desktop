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

use super::scroll_loaded_items::LoadedItems;
use super::scroll_search_step::SearchStep;
use super::scroll_signals::ScrollSignals;
use crate::tree::AXElement;

const SETTLE_READS: usize = 8;
const SETTLE_PAUSE: Duration = Duration::from_millis(25);
const INSIDE_DEPTH: usize = 2;
const INSIDE_CHILDREN: usize = 16;
const INSIDE_READS: usize = 32;

pub(crate) fn page_scroll_area(
    element: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<ScrollPage, AdapterError> {
    let area = search_area(element, deadline)?;
    let room = bar_room(&area, direction, deadline)?;
    if room == Some(false) {
        return Ok(ScrollPage::AtEnd { delivered: false });
    }
    let before = super::scroll_page::capture(&area, direction, deadline);
    let anchor = match room {
        None => backward_anchor(&area, direction, deadline),
        Some(_) => None,
    };
    let mut farthest = || reveal_farthest_item(&area, direction, deadline);
    let mut page_part = || press_bar_part(&area, direction, deadline);
    let mut page_action =
        || Ok(super::try_action(&area, super::page_action(direction), deadline).into());
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
            let page = settle(
                before,
                || super::scroll_page::capture(&area, direction, deadline),
                || pause(deadline),
            );
            match (&page, &anchor) {
                (ScrollPage::Moved, Some((item, was))) => {
                    match super::scroll_search_value::item_offset(item, direction, deadline) {
                        Some(now)
                            if super::scroll_search_value::moved_backward(*was, now, direction) =>
                        {
                            ScrollPage::AtEnd { delivered: true }
                        }
                        _ => page,
                    }
                }
                _ => page,
            }
        },
    )
}

/// A loaded item and its position, read before the steps of an area with no
/// readable bar. Its frames say only that something moved; this item says
/// which way. A page action at the end of a SwiftUI lazy stack scrolls back
/// several rows, which is no page toward the end.
fn backward_anchor(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Option<(AXElement, f64)> {
    let items = LoadedItems::read(area, deadline).ok()?;
    let item = items.middle()?.clone();
    let offset = super::scroll_search_value::item_offset(&item, direction, deadline)?;
    Some((item, offset))
}

/// Sends steps in order until one moves the area, and settles what it did.
/// A step that failed before it sent anything, was refused before delivery,
/// or was refused with the area proven unmoved passes to the next step, and
/// so does a sent step that moved nothing: an item already in view reveals
/// with no movement, so one unmoved step is no end. The page is the end only
/// when steps were sent, none moved the area, and its bar does not say it
/// has room; with room left that is an inert area. A refusal whose effect
/// the reads cannot decide ends the page with that error.
pub(super) fn send_steps(
    steps: &mut [&mut dyn FnMut() -> Result<SearchStep, AdapterError>],
    room_left: bool,
    mut settled: impl FnMut() -> ScrollPage,
) -> Result<ScrollPage, AdapterError> {
    let mut refusal = None;
    let mut unmoved = false;
    for step in steps.iter_mut() {
        match step() {
            Err(unsent) => refusal = Some(unsent),
            Ok(SearchStep::Skipped) => {}
            Ok(SearchStep::Sent) => match settled() {
                ScrollPage::AtEnd { .. } => unmoved = true,
                page => return Ok(page),
            },
            Ok(SearchStep::Refused(error))
                if super::scroll_into_view::declined_without_delivery(&error) =>
            {
                refusal = Some(error);
            }
            Ok(SearchStep::Refused(error)) => match settled() {
                ScrollPage::Moved => return Ok(ScrollPage::Moved),
                ScrollPage::AtEnd { .. } => refusal = Some(error),
                ScrollPage::Unconfirmed => return Err(error),
            },
        }
    }
    match (unmoved, room_left) {
        (true, true) => Err(inert_area()),
        (true, false) => Ok(ScrollPage::AtEnd { delivered: true }),
        (false, _) => Err(refusal.unwrap_or_else(no_page_step)),
    }
}

/// The scroll area a search pages: the root itself, or else the one around
/// a list root, or else one inside any other root, such as a pane that holds
/// a search field beside its list.
fn search_area(element: &AXElement, deadline: Deadline) -> Result<AXElement, AdapterError> {
    let role = super::scroll_read::string(element, "AXRole", deadline)?;
    if role.as_deref() == Some("AXScrollArea") {
        return Ok(element.clone());
    }
    let outside = |deadline| super::find_scroll_area(element, deadline);
    let inside = |deadline| scroll_area_inside(element, deadline);
    let found = if looks_inside_first(role.as_deref()) {
        match inside(deadline)? {
            Some(area) => Some(area),
            None => outside(deadline)?,
        }
    } else {
        match outside(deadline)? {
            Some(area) => Some(area),
            None => inside(deadline)?,
        }
    };
    Ok(found.unwrap_or_else(|| element.clone()))
}

/// A list root is paged by the area around it; a row inside it may hold a
/// scroll area of its own. Any other root may wrap the area it means.
pub(super) fn looks_inside_first(role: Option<&str>) -> bool {
    !role.is_some_and(|role| super::scroll_loaded_items::COLLECTION_ROLES.contains(&role))
}

fn scroll_area_inside(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let mut level = vec![element.clone()];
    let mut reads = 0;
    for _ in 0..INSIDE_DEPTH {
        let mut next = Vec::new();
        for parent in &level {
            for child in super::scroll_read::children_up_to(parent, INSIDE_CHILDREN, deadline)? {
                if reads == INSIDE_READS {
                    return Ok(None);
                }
                reads += 1;
                if super::scroll_read::string(&child, "AXRole", deadline)?.as_deref()
                    == Some("AXScrollArea")
                {
                    return Ok(Some(child));
                }
                next.push(child);
            }
        }
        level = next;
    }
    Ok(None)
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
) -> Result<SearchStep, AdapterError> {
    match LoadedItems::read(area, deadline)?.farthest(direction, deadline)? {
        Some(item) => Ok(super::try_action(&item, "AXScrollToVisible", deadline).into()),
        None => Ok(SearchStep::Skipped),
    }
}

fn press_bar_part(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<SearchStep, AdapterError> {
    let (bar_attribute, _) = super::scroll_bar_action(direction);
    let Some(bar) = super::scroll_read::element(area, bar_attribute, deadline)? else {
        return Ok(SearchStep::Skipped);
    };
    let part = match direction {
        Direction::Down | Direction::Right => "AXIncrementPage",
        Direction::Up | Direction::Left => "AXDecrementPage",
    };
    for child in super::scroll_read::children(&bar, deadline)? {
        if super::scroll_read::string(&child, "AXSubrole", deadline)?.as_deref() == Some(part) {
            return Ok(super::try_action(&child, "AXPress", deadline).into());
        }
    }
    Ok(SearchStep::Skipped)
}

fn bar_line_step(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<SearchStep, AdapterError> {
    let (bar_attribute, increment) = super::scroll_bar_action(direction);
    match super::scroll_read::element(area, bar_attribute, deadline)? {
        Some(bar) => Ok(super::try_action(&bar, increment, deadline).into()),
        None => Ok(SearchStep::Skipped),
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

pub(super) fn pause(deadline: Deadline) -> bool {
    let Ok(slice) = deadline.remaining_slice(SETTLE_PAUSE) else {
        return false;
    };
    std::thread::sleep(slice.min(SETTLE_PAUSE));
    true
}

#[cfg(test)]
#[path = "scroll_search_tests.rs"]
mod tests;
