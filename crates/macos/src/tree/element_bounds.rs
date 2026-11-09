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

/// A sheet or popover is a window of its own. The window it hangs from does not
/// clip it, so its frame replaces the inherited viewport instead of narrowing it:
/// a save sheet wider than its document window keeps its buttons visible.
pub(crate) fn draws_own_surface(role: &str) -> bool {
    matches!(role, "AXSheet" | "AXPopover")
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
    if draws_own_surface(role) {
        return bounds;
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
    if let Some(window) = owning_window_bounds(element, deadline)? {
        return clip_by_ancestors(element, window, instant);
    }
    let Ok(Some(window)) = window_ancestor_bounds(element, instant) else {
        return Ok(None);
    };
    Ok(clip_by_ancestors(element, window, instant).ok().flatten())
}

fn clip_by_ancestors(
    element: &AXElement,
    window: Rect,
    instant: std::time::Instant,
) -> Result<Option<Rect>, AdapterError> {
    let mut clips = Vec::new();
    let mut current = super::surface_read::element(element, "AXParent", instant)?;
    for _ in 0..super::properties::element::ABSOLUTE_MAX_DEPTH {
        let Some(parent) = current else {
            return Ok(viewport_from_clips(window, &clips));
        };
        let Some(role) = super::surface_read::string(&parent, "AXRole", instant)? else {
            return Ok(None);
        };
        if role == "AXApplication" {
            return Ok(viewport_from_clips(window, &clips));
        }
        if clips_descendants(&role) {
            clips.push((role.clone(), read_bounds_with_deadline(&parent, instant)?));
        }
        if role == "AXWindow" || draws_own_surface(&role) {
            return Ok(viewport_from_clips(window, &clips));
        }
        current = super::surface_read::element(&parent, "AXParent", instant)?;
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Clipping ancestry exceeded its bounded depth",
    ))
}

/// Folds the clipping ancestors met walking up from an element, nearest first,
/// into its viewport. The walk stops at the first window, sheet or popover; a
/// sheet or popover ends the list and its frame replaces the window's.
pub(crate) fn viewport_from_clips(window: Rect, clips: &[(String, Option<Rect>)]) -> Option<Rect> {
    let own_surface = clips.iter().position(|(role, _)| draws_own_surface(role));
    let mut viewport = match own_surface {
        Some(index) => clips[index].1,
        None => Some(window),
    };
    let below = own_surface.map_or(clips, |index| &clips[..index]);
    for (role, bounds) in below {
        viewport = clipped_viewport(viewport, role, *bounds);
    }
    viewport
}

fn window_ancestor_bounds(
    element: &AXElement,
    instant: std::time::Instant,
) -> Result<Option<Rect>, AdapterError> {
    let window = first_window_ancestor(
        element,
        |current| super::surface_read::element(current, "AXParent", instant),
        |current| super::surface_read::string(current, "AXRole", instant),
    )?;
    match window {
        Some(window) => read_bounds_with_deadline(&window, instant),
        None => Ok(None),
    }
}

/// Roles that draw on their own surface, outside the window their parent chain
/// passes through. Content at or below one of them has no window viewport,
/// because an open menu or popover would otherwise be judged offscreen.
const OWN_SURFACE_ROLES: [&str; 5] = [
    "AXMenu",
    "AXMenuItem",
    "AXMenuBarItem",
    "AXMenuBar",
    "AXPopover",
];

/// Some controls, such as System Settings popup buttons, answer
/// `kAXErrorNoValue` for `AXWindow` although an `AXWindow` encloses them. The
/// parent chain still reaches that window. The search starts at the element
/// itself, stops without a window at the first own-surface role or at the
/// application, and a chain that cannot be read leaves the viewport unknown,
/// as it was before the fallback existed.
fn first_window_ancestor<E: Clone>(
    start: &E,
    mut parent: impl FnMut(&E) -> Result<Option<E>, AdapterError>,
    mut role: impl FnMut(&E) -> Result<Option<String>, AdapterError>,
) -> Result<Option<E>, AdapterError> {
    let mut current = Some(start.clone());
    for _ in 0..super::properties::element::ABSOLUTE_MAX_DEPTH {
        let Some(ancestor) = current else {
            return Ok(None);
        };
        match role(&ancestor)?.as_deref() {
            Some("AXWindow") => return Ok(Some(ancestor)),
            Some(own) if OWN_SURFACE_ROLES.contains(&own) => return Ok(None),
            Some("AXApplication") | None => return Ok(None),
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
/// that reports every open menu item as offscreen. Only `AXWindow` is read
/// here; an element that publishes none is handled by the parent-chain
/// fallback in `ancestor_viewport`.
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
