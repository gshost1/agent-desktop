use agent_desktop_core::{AdapterError, Deadline, ErrorCode, WindowInfo};

use super::window_enum::enumerate_top_level;
use super::window_identity::{WindowIdentityEvidence, live_window_title};
use super::window_ops::{is_foreground_window, parse_handle, passes_filter, window_state};
use crate::system::shell::shell_surface::presented_surface_at;

/// Resolves a live window by `WindowInfo.id`, corroborating pid and process
/// generation against the handle's current owner (stored-evidence rule).
///
/// A window the agent inventory leaves out resolves only when it is a shell
/// surface the shell-surface resolver presents right now under the same id
/// and process, which is where a shell-surface snapshot's window id came
/// from.
pub(crate) fn resolve_window_strict(
    expected: &WindowInfo,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    deadline.ensure_remaining()?;
    let handle = parse_handle(&expected.id);
    if handle.is_null() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "window id is not a Windows HWND-shaped id",
        ));
    }
    if !window_exists(handle) {
        return Err(AdapterError::new(
            ErrorCode::WindowNotFound,
            "window handle is no longer a live top-level window",
        ));
    }
    let Some(evidence) = WindowIdentityEvidence::from_info(handle, expected) else {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "headed focus requires a process-instance token on the stored window",
        ));
    };
    evidence.verify_stored()?;
    live_window_info(handle, expected, deadline)
}

fn live_window_info(
    handle: super::window_enum::WindowHandle,
    expected: &WindowInfo,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    let mut found = None;
    enumerate_top_level(|window| {
        if window.handle != handle {
            return true;
        }
        found = Some(window);
        false
    })?;
    let refusal = match found {
        Some(window) if passes_filter(&window) || window.iconic => {
            let focused = is_foreground_window(handle);
            return Ok(confirmed(
                expected,
                live_window_title(handle),
                Some(window.rect),
                window_state(focused, window.iconic, window.visible),
            ));
        }
        Some(_) => "window is no longer an agent-visible top-level window",
        None => "window handle is no longer present in the top-level inventory",
    };
    match presented_surface_at(handle, deadline)? {
        Some((surface, _)) if surface.pid == expected.pid => Ok(confirmed(
            expected,
            Some(surface.title).filter(|title| !title.is_empty()),
            surface.bounds,
            surface.state,
        )),
        _ => Err(AdapterError::new(ErrorCode::WindowNotFound, refusal)),
    }
}

/// The resolved window: the stored identity with the live title (the stored
/// one when the live read has none), bounds and state.
fn confirmed(
    expected: &WindowInfo,
    title: Option<String>,
    bounds: Option<agent_desktop_core::Rect>,
    state: agent_desktop_core::WindowState,
) -> WindowInfo {
    WindowInfo {
        id: expected.id.clone(),
        title: title.unwrap_or_else(|| expected.title.clone()),
        app: expected.app.clone(),
        pid: expected.pid,
        process_instance: expected.process_instance.clone(),
        bounds,
        state,
    }
}

#[cfg(target_os = "windows")]
fn window_exists(handle: super::window_enum::WindowHandle) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;
    unsafe { IsWindow(handle) != 0 }
}

#[cfg(not(target_os = "windows"))]
fn window_exists(_handle: super::window_enum::WindowHandle) -> bool {
    false
}

#[cfg(test)]
#[path = "window_resolve_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "window_resolve_shell_tests.rs"]
mod shell_tests;
