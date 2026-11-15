//! A scroll bar value step sized to the viewport, for a search area that
//! moves only when its bar's value is written.
//!
//! A bar's value is a fraction of a content length the accessibility tree
//! does not report, and a lazy list only estimates. So the step measures
//! it: a tiny write, the distance one loaded item moved, then a write that
//! moves the content by a fixed share of the viewport.

use agent_desktop_core::{AdapterError, Deadline, Direction, Rect};

use crate::tree::AXElement;

const PROBE_SHIFT: f64 = 0.002;
const VIEWPORT_SHARE: f64 = 0.8;
const MIN_MEASURED_PIXELS: f64 = 0.5;

pub(super) fn calibrated_value_step(
    area: &AXElement,
    direction: &Direction,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let (bar_attribute, _) = super::scroll_bar_action(direction);
    let Some(bar) = super::scroll_read::element(area, bar_attribute, deadline)? else {
        return Ok(false);
    };
    let Some(start) = super::scroll_read::number(&bar, "AXValue", deadline)? else {
        return Ok(false);
    };
    let items = super::scroll_search::loaded_items(area, deadline)?;
    let Some(probe) = items.get(items.len() / 2) else {
        return Ok(false);
    };
    let (Some(viewport), Some(before)) = (bounds(area, deadline)?, bounds(probe, deadline)?) else {
        return Ok(false);
    };
    if !write_value(&bar, shifted(start, PROBE_SHIFT, direction), deadline)? {
        return Ok(false);
    }
    let Ok(Some(after)) = bounds(probe, deadline) else {
        return Ok(true);
    };
    let measured = (offset(&after, direction) - offset(&before, direction)).abs();
    if let Some(page) = page_shift(measured, length(&viewport, direction)) {
        write_value(&bar, shifted(start, page, direction), deadline)?;
    }
    Ok(true)
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
