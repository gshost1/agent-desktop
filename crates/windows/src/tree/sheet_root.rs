//! The modal sheet a named window presents, for `--surface sheet`.
//!
//! A sheet is read from the window the caller named, never from whichever
//! window holds the foreground, so a background application's sheet is found
//! and no read enumerates the desktop's windows. A Chromium modal reports
//! `WindowIsModal` on the window itself; a Win32 modal dialog is a separate
//! top-level window the named window owns and stays enabled while its owner
//! is disabled, which is what `GW_ENABLEDPOPUP` returns.

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, SnapshotSurface};

use super::automation::root_from_hwnd;
use super::element::UIAElement;
use super::properties::property_ids::TreeProperty;
use super::properties::read_one;
use super::surface_inventory::modal_classification;

/// The named window when it is modal, else the modal popup it owns, else a
/// complete `surface_absent` answer. A `WindowIsModal` read that failed is
/// no answer and is returned as the error it is, never read as absent.
pub(crate) fn sheet_root(handle: isize, deadline: Deadline) -> Result<UIAElement, AdapterError> {
    let window = root_from_hwnd(handle, deadline)?;
    if is_modal(&window)? {
        return Ok(window);
    }
    if let Some(popup) = owned_enabled_popup(handle) {
        match root_from_hwnd(popup, deadline) {
            Ok(element) if is_modal(&element)? => return Ok(element),
            Ok(_) => {}
            Err(error) if error.code == ErrorCode::WindowNotFound => {}
            Err(error) => return Err(error),
        }
    }
    Err(AdapterError::surface_absent(
        SnapshotSurface::Sheet,
        "The window presents no modal sheet",
    ))
}

fn is_modal(element: &UIAElement) -> Result<bool, AdapterError> {
    modal_classification(read_one(element, TreeProperty::WindowIsModal))
}

/// The visible, enabled popup the window owns, when the same process owns
/// both. The window itself, a hidden popup or a popup of another process is
/// not one.
#[cfg(target_os = "windows")]
fn owned_enabled_popup(handle: isize) -> Option<isize> {
    use crate::system::window::window_identity::live_window_owner;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GW_ENABLEDPOPUP, GW_OWNER, GetWindow, IsWindowVisible,
    };

    let owner = handle as crate::system::window::window_enum::WindowHandle;
    let popup = unsafe { GetWindow(owner, GW_ENABLEDPOPUP) };
    if popup.is_null() || popup == owner {
        return None;
    }
    let owned = unsafe { GetWindow(popup, GW_OWNER) } == owner;
    let visible = unsafe { IsWindowVisible(popup) } != 0;
    let process = live_window_owner(owner);
    let same_process = process.is_some() && live_window_owner(popup) == process;
    (owned && visible && same_process).then_some(popup as isize)
}

#[cfg(not(target_os = "windows"))]
fn owned_enabled_popup(_handle: isize) -> Option<isize> {
    None
}
