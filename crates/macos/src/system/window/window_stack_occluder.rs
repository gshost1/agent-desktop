//! Reads the window server's front-to-back window stack at a screen point.
//!
//! When the accessibility hit test cannot name what sits under a point, the
//! window server still knows which on-screen window receives a click there.
//! A window of another process in front of the target's own window is
//! positive evidence that the point is covered. Only the owner process id,
//! the layer, the alpha and the bounds are read, never a title or an owner
//! name.
//!
//! Windows at the Dock level and above are system chrome: the Dock and the
//! screenshot tool keep transparent, click-through windows over the whole
//! screen there, so they are passed over rather than read as occluders.

use agent_desktop_core::{AdapterError, Point, Rect, adapter::hit_test::HitTestResult};

use super::cg_window::{WindowDictionary, int_field, int_or_float_field, rect_field};

/// `kCGDockWindowLevel`. Normal, floating, modal-panel and utility windows sit
/// below it.
pub(crate) const DOCK_WINDOW_LEVEL: i64 = 20;

/// The window server's answer for a hit test accessibility could not decide:
/// the covering window as the interceptor, or `None` when there is no
/// evidence or the window list cannot be read.
pub(crate) fn intercepting_window(point: &Point, target_pid: i32) -> Option<HitTestResult> {
    let bounds = foreign_window_at(point, target_pid).ok()??;
    Some(HitTestResult::InterceptedBy {
        role: Some("window".to_string()),
        name: None,
        bounds: Some(bounds),
    })
}

/// The bounds of the window that covers `point` in front of every window of
/// `target_pid`, when another process owns it. `None` means the window server
/// gives no such evidence.
fn foreign_window_at(point: &Point, target_pid: i32) -> Result<Option<Rect>, AdapterError> {
    let dictionaries = super::popup_menu_window::on_screen_dictionaries()?;
    Ok(front_foreign_window(
        &stack_from(&dictionaries),
        point,
        target_pid,
        crate::system::process::legacy_host::is_agent_desktop,
    ))
}

/// The visible windows of the stack, front to back, as owner pid, layer and
/// bounds. A window with no alpha, or with a field missing, draws nothing that
/// can receive a click.
pub(in crate::system) fn stack_from(dictionaries: &[WindowDictionary]) -> Vec<(i32, i64, Rect)> {
    dictionaries
        .iter()
        .filter(|dictionary| {
            int_or_float_field(dictionary, "kCGWindowAlpha").is_some_and(|alpha| alpha > 0.0)
        })
        .filter_map(|dictionary| {
            let pid = i32::try_from(int_field(dictionary, "kCGWindowOwnerPID")?).ok()?;
            let layer = int_field(dictionary, "kCGWindowLayer")?;
            Some((pid, layer, rect_field(dictionary, "kCGWindowBounds")?))
        })
        .collect()
}

/// Walks the stack front to back over the windows that contain `point`. The
/// first window of `target_pid` ends the walk with no evidence. A window of
/// another process below the Dock level is the occluder. agent-desktop's own
/// overlay windows ignore the mouse and are passed over, as is system chrome.
pub(in crate::system) fn front_foreign_window(
    stack: &[(i32, i64, Rect)],
    point: &Point,
    target_pid: i32,
    is_agent_desktop: impl Fn(i32) -> bool,
) -> Option<Rect> {
    for (pid, layer, bounds) in stack {
        if !contains(bounds, point) {
            continue;
        }
        if *pid == target_pid {
            return None;
        }
        if (0..DOCK_WINDOW_LEVEL).contains(layer) && !is_agent_desktop(*pid) {
            return Some(*bounds);
        }
    }
    None
}

fn contains(bounds: &Rect, point: &Point) -> bool {
    bounds.width > 0.0
        && bounds.height > 0.0
        && point.x >= bounds.x
        && point.y >= bounds.y
        && point.x < bounds.x + bounds.width
        && point.y < bounds.y + bounds.height
}

#[cfg(test)]
#[path = "window_stack_occluder_tests.rs"]
mod tests;
