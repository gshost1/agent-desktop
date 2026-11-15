//! One page of a scroll area for a search whose target has not loaded yet.
//!
//! It sends the same step as revealing a known element: `ax_scroll` on the
//! area, refused at the scroll bar's limit. A reveal reads movement from the
//! target's bounds; a search has no target yet, so it reads the area's own
//! scroll signals: the bar's value and the frames of its first children.

use agent_desktop_core::{AdapterError, Deadline, Direction, InteractionPolicy, ScrollPage};
use std::time::Duration;

use super::scroll_signals::ScrollSignals;
use crate::tree::AXElement;

const SETTLE_READS: usize = 8;
const SETTLE_PAUSE: Duration = Duration::from_millis(25);

pub(crate) fn page_scroll_area(
    element: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<ScrollPage, AdapterError> {
    let area = super::find_scroll_area(element, deadline)?.unwrap_or_else(|| element.clone());
    if super::scroll_into_view::bar_at_limit(&area, direction, deadline)? {
        return Ok(ScrollPage::AtEnd { delivered: false });
    }
    let before = super::scroll_page::capture(&area, direction, deadline);
    match super::ax_scroll(&area, direction, 1, InteractionPolicy::headless(), deadline) {
        Ok(_) => {}
        Err(error) if super::scroll_into_view::declined_without_delivery(&error) => {
            return Ok(ScrollPage::AtEnd { delivered: false });
        }
        Err(error) => return Err(error),
    }
    Ok(settle(
        before,
        || super::scroll_page::capture(&area, direction, deadline),
        || pause(deadline),
    ))
}

/// Reads the area until a signal moves, which settles the page as moved.
/// Signals that stay readable and unchanged through the last read settle it
/// as the end of the area. Anything the reads cannot decide is unconfirmed.
pub(super) fn settle(
    before: ScrollSignals,
    mut read: impl FnMut() -> ScrollSignals,
    mut pause: impl FnMut() -> bool,
) -> ScrollPage {
    let mut unmoved = false;
    for attempt in 0..SETTLE_READS {
        match before.movement(read()) {
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
