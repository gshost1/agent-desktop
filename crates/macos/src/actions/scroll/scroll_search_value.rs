//! A scroll bar value step sized to the viewport, for a search area that
//! moves only when its bar's value is written.
//!
//! A bar's value is a fraction of a content length the accessibility tree
//! does not report, and a lazy list only estimates. So the step measures
//! it: a tiny write, the distance one loaded item moved, then a write that
//! moves the content by a fixed share of the viewport.

use agent_desktop_core::{AdapterError, Deadline, Direction, ErrorCode, Rect};

use super::scroll_loaded_items::LoadedItems;
use super::scroll_search_step::SearchStep;
use crate::tree::AXElement;

const PROBE_SHIFT: f64 = 0.002;
const VIEWPORT_SHARE: f64 = 0.8;
const MIN_MEASURED_PIXELS: f64 = 0.5;
const PROBE_READS: usize = 8;

pub(super) fn calibrated_value_step(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<SearchStep, AdapterError> {
    let (bar_attribute, _) = super::scroll_bar_action(direction);
    let Some(bar) = super::scroll_read::element(area, bar_attribute, deadline)? else {
        return Ok(SearchStep::Skipped);
    };
    let Some(start) = super::scroll_read::number(&bar, "AXValue", deadline)? else {
        return Ok(SearchStep::Skipped);
    };
    let items = LoadedItems::read(area, deadline)?;
    let Some(probe) = items.middle() else {
        return Ok(SearchStep::Skipped);
    };
    let (Some(viewport), Some(before)) = (bounds(area, deadline)?, bounds(probe, deadline)?) else {
        return Ok(SearchStep::Skipped);
    };
    let mut reads = 0;
    let moved = || {
        if reads > 0 && !super::scroll_search::pause(deadline) {
            return None;
        }
        reads += 1;
        let after = bounds(probe, deadline).ok().flatten()?;
        Some((offset(&after, direction) - offset(&before, direction)).abs())
    };
    Ok(calibrate(
        (start, length(&viewport, direction)),
        direction,
        |value| write_value(&bar, value, deadline),
        moved,
    ))
}

/// Writes a probe shift, waits for the probe item to move, then writes the
/// shift that moves the content by the viewport share. A probe the reads
/// never see move sizes no page, and a page write that fails or is not
/// taken leaves the probe in place, so in both cases the bar is put back
/// where it started first: a probe shift left in place would read as a page
/// that moved a fraction of a pixel. A bar that cannot be put back is
/// displaced, never a sent page.
pub(super) fn calibrate(
    (start, viewport): (f64, f64),
    direction: &Direction,
    mut write: impl FnMut(f64) -> Result<bool, AdapterError>,
    mut moved: impl FnMut() -> Option<f64>,
) -> SearchStep {
    match write(shifted(start, PROBE_SHIFT, direction)) {
        Ok(true) => {}
        unsent => return unsent.into(),
    }
    let measured =
        (0..PROBE_READS).find_map(|_| moved().filter(|pixels| *pixels >= MIN_MEASURED_PIXELS));
    if let Some(page) = measured.and_then(|pixels| page_shift(pixels, viewport)) {
        match write(shifted(start, page, direction)) {
            Ok(true) => return SearchStep::Sent,
            Ok(false) => {}
            Err(error) => return restored(write(start), SearchStep::Refused(error)),
        }
    }
    restored(write(start), SearchStep::Skipped)
}

/// What the step amounts to once the probe was to be put back: `after` when
/// the restore was taken, otherwise a displaced bar, whose error cannot
/// claim nothing was sent because the probe write was.
fn restored(restore: Result<bool, AdapterError>, after: SearchStep) -> SearchStep {
    match restore {
        Ok(true) => after,
        Ok(false) => SearchStep::Displaced(probe_left_in_place()),
        Err(error) => SearchStep::Displaced(
            crate::actions::DeliveryTracker::from_delivered_units(1).annotate(error),
        ),
    }
}

fn probe_left_in_place() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "The scroll bar did not take back the probe step a search page measured with",
    )
    .with_details(serde_json::json!({ "kind": "scroll_search_probe_not_restored" }))
    .with_suggestion(
        "The list moved by a small step. Take a fresh snapshot and read the list before searching again.",
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::delivered_unverified())
}

/// The value change that moves the content by the viewport share, from the
/// pixels the probe write moved it; `None` when the probe moved too little
/// to measure.
pub(super) fn page_shift(measured_pixels: f64, viewport_length: f64) -> Option<f64> {
    (measured_pixels >= MIN_MEASURED_PIXELS && viewport_length > 0.0)
        .then(|| PROBE_SHIFT * viewport_length * VIEWPORT_SHARE / measured_pixels)
}

pub(super) fn shifted(start: f64, shift: f64, direction: &Direction) -> f64 {
    match direction {
        Direction::Down | Direction::Right => (start + shift).min(1.0),
        Direction::Up | Direction::Left => (start - shift).max(0.0),
    }
}

/// Where `item` sits along `direction`'s axis, when its bounds can be read.
pub(super) fn item_offset(
    item: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Option<f64> {
    bounds(item, deadline)
        .ok()
        .flatten()
        .map(|rect| offset(&rect, direction))
}

/// Whether an item that stayed loaded moved the way content moves when the
/// area scrolls back against `direction`: down the screen for a search down.
pub(super) fn moved_backward(before: f64, after: f64, direction: &Direction) -> bool {
    match direction {
        Direction::Down | Direction::Right => after > before + MIN_MEASURED_PIXELS,
        Direction::Up | Direction::Left => after < before - MIN_MEASURED_PIXELS,
    }
}

fn offset(rect: &Rect, direction: &Direction) -> f64 {
    match direction {
        Direction::Up | Direction::Down => rect.y,
        Direction::Left | Direction::Right => rect.x,
    }
}

fn length(rect: &Rect, direction: &Direction) -> f64 {
    match direction {
        Direction::Up | Direction::Down => rect.height,
        Direction::Left | Direction::Right => rect.width,
    }
}

fn bounds(element: &AXElement, deadline: Deadline) -> Result<Option<Rect>, AdapterError> {
    crate::tree::element_bounds::read_bounds_with_deadline(
        element,
        crate::tree::locator_deadline::from_operation(deadline)?,
    )
}

fn write_value(bar: &AXElement, value: f64, deadline: Deadline) -> Result<bool, AdapterError> {
    use core_foundation::{base::TCFType, number::CFNumber, string::CFString};

    super::prepare(bar, deadline)?;
    let number = CFNumber::from(value as f32);
    let attribute = CFString::new("AXValue");
    let error = crate::tree::properties::ax_ipc::set_attribute_value(
        bar,
        attribute.as_concrete_TypeRef(),
        number.as_CFTypeRef(),
        deadline,
    )?;
    crate::actions::ax_mutation::classify_result("AXValue", "AXUIElementSetAttributeValue", error)
}

#[cfg(test)]
#[path = "scroll_search_value_tests.rs"]
mod tests;
