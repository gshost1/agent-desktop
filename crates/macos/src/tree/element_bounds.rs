use agent_desktop_core::{AdapterError, Deadline, ErrorCode, Rect};

use super::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn rect_from_parts(
    point: core_graphics::geometry::CGPoint,
    size: core_graphics::geometry::CGSize,
) -> Option<Rect> {
    Rect {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    }
    .validate()
    .ok()
}

#[cfg(target_os = "macos")]
pub(crate) fn read_bounds_with_deadline(
    element: &AXElement,
    deadline: std::time::Instant,
) -> Result<Option<Rect>, agent_desktop_core::AdapterError> {
    let point = super::resolve::resolve_ax_read::read_point(element, "AXPosition", deadline)?;
    let size = super::resolve::resolve_ax_read::read_size(element, "AXSize", deadline)?;
    Ok(point
        .zip(size)
        .and_then(|(point, size)| rect_from_parts(point, size)))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn read_bounds_with_deadline(
    _element: &AXElement,
    _deadline: std::time::Instant,
) -> Result<Option<Rect>, agent_desktop_core::AdapterError> {
    Ok(None)
}

pub(crate) fn owning_window_bounds(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(element, deadline)?;
    let window = first_owning_container(|attribute| {
        crate::tree::properties::attributes::copy_element_attr_result(element, attribute, deadline)
    })
    .map_err(|(attribute, error)| read_error(attribute, error))?;
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    let Some(window) = window else {
        return Ok(None);
    };
    crate::tree::element_bounds::read_bounds_with_deadline(
        &window,
        super::locator_deadline::from_operation(deadline)?,
    )
}

pub(crate) fn clips_descendants(role: &str) -> bool {
    matches!(
        role,
        "AXWindow" | "AXScrollArea" | "AXWebArea" | "AXSheet" | "AXPopover"
    )
}

pub(crate) fn intersect_rects(left: Rect, right: Rect) -> Option<Rect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let width = (left.x + left.width).min(right.x + right.width) - x;
    let height = (left.y + left.height).min(right.y + right.height) - y;
    (width > 0.0 && height > 0.0).then_some(Rect {
        x,
        y,
        width,
        height,
    })
}

pub(crate) fn clipped_viewport(
    parent: Option<Rect>,
    role: &str,
    bounds: Option<Rect>,
) -> Option<Rect> {
    if !clips_descendants(role) {
        return parent;
    }
    let parent = parent?;
    let bounds = bounds?;
    Some(intersect_rects(parent, bounds).unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }))
}

pub(crate) fn ancestor_viewport(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    let instant = super::locator_deadline::from_operation(deadline)?;
    let mut viewport = match owning_window_bounds(element, deadline)? {
        Some(bounds) => Some(bounds),
        None => window_ancestor_bounds(element, instant)?,
    };
    if viewport.is_none() {
        return Ok(None);
    }
    let mut current = super::surface_read::element(element, "AXParent", instant)?;
    for _ in 0..super::properties::element::ABSOLUTE_MAX_DEPTH {
        let Some(parent) = current else {
            return Ok(viewport);
        };
        let Some(role) = super::surface_read::string(&parent, "AXRole", instant)? else {
            return Ok(None);
        };
        if role == "AXApplication" {
            return Ok(viewport);
        }
        if clips_descendants(&role) {
            viewport = clipped_viewport(
                viewport,
                &role,
                read_bounds_with_deadline(&parent, instant)?,
            );
        }
        if role == "AXWindow" {
            return Ok(viewport);
        }
        current = super::surface_read::element(&parent, "AXParent", instant)?;
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Clipping ancestry exceeded its bounded depth",
    ))
}

fn window_ancestor_bounds(
    element: &AXElement,
    instant: std::time::Instant,
) -> Result<Option<Rect>, AdapterError> {
    let window = first_window_ancestor(
        element.clone(),
        |current| super::surface_read::element(current, "AXParent", instant),
        |current| super::surface_read::string(current, "AXRole", instant),
    )?;
    match window {
        Some(window) => read_bounds_with_deadline(&window, instant),
        None => Ok(None),
    }
}

/// Some controls, such as System Settings popup buttons, answer
/// `kAXErrorNoValue` for `AXWindow` although an `AXWindow` encloses them. The
/// parent chain still reaches that window. Menu content has no window even
/// when its chain passes through one, because an open menu draws outside the
/// window and would be judged offscreen; the search stops at the first menu
/// role, and a chain that ends at the application has no window either.
fn first_window_ancestor<E>(
    start: E,
    mut parent: impl FnMut(&E) -> Result<Option<E>, AdapterError>,
    mut role: impl FnMut(&E) -> Result<Option<String>, AdapterError>,
) -> Result<Option<E>, AdapterError> {
    let mut current = parent(&start)?;
    for _ in 0..super::properties::element::ABSOLUTE_MAX_DEPTH {
        let Some(ancestor) = current else {
            return Ok(None);
        };
        match role(&ancestor)?.as_deref() {
            Some("AXWindow") => return Ok(Some(ancestor)),
            Some("AXApplication" | "AXMenu" | "AXMenuItem" | "AXMenuBarItem" | "AXMenuBar")
            | None => return Ok(None),
            Some(_) => current = parent(&ancestor)?,
        }
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Window ancestry exceeded its bounded depth",
    ))
}

/// The viewport an element is clipped by. `AXTopLevelUIElement` is not a
/// substitute: for menu content it resolves to the menu bar, a 29-point strip
/// that reports every open menu item as offscreen. An element with no window is
/// drawn on its own surface, so its clipping viewport is simply unknown.
fn first_owning_container(
    mut read: impl FnMut(&'static str) -> Result<Option<AXElement>, i32>,
) -> Result<Option<AXElement>, (&'static str, i32)> {
    read("AXWindow").map_err(|error| ("AXWindow", error))
}

fn read_error(attribute: &str, error: i32) -> AdapterError {
    AdapterError::new(
        if error == accessibility_sys::kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == accessibility_sys::kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == accessibility_sys::kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        },
        format!("Could not read {attribute} for live state"),
    )
    .with_details(serde_json::json!({ "attribute": attribute, "ax_error": error }))
}

#[cfg(test)]
#[path = "element_bounds_tests.rs"]
mod tests;
