//! The system-wide accessibility element, read only once this process has a
//! window-server connection.
//!
//! A process with no window-server connection gets `kAXErrorCannotComplete`
//! at once from every system-wide read, such as `AXFocusedApplication`, and
//! retrying does not help: in fresh processes, eleven reads 10 ms apart all
//! failed. Any window-list call opens the connection, after which the first
//! read succeeds. Every command is a fresh process, so the first read of the
//! focused application failed in every one of them until something else
//! happened to list windows. A window-id list is the cheapest public call
//! that opens the connection, and the later window reads reuse it.

use std::sync::OnceLock;

const ON_SCREEN_ONLY: u32 = 1 << 0;
const NO_RELATIVE_WINDOW: u32 = 0;

static WINDOW_SERVER_CONNECTION: OnceLock<()> = OnceLock::new();

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCreate(
        option: u32,
        relative_to_window: u32,
    ) -> core_foundation_sys::array::CFArrayRef;
}

/// Opens this process's window-server connection, then returns the
/// system-wide accessibility element.
pub(crate) fn element() -> crate::tree::AXElement {
    open_window_server_connection();
    crate::tree::AXElement(unsafe { accessibility_sys::AXUIElementCreateSystemWide() })
}

fn open_window_server_connection() {
    WINDOW_SERVER_CONNECTION.get_or_init(|| {
        let windows = unsafe { CGWindowListCreate(ON_SCREEN_ONLY, NO_RELATIVE_WINDOW) };
        if !windows.is_null() {
            unsafe { core_foundation::base::CFRelease(windows.cast()) };
        }
    });
}

#[cfg(test)]
#[path = "system_wide_tests.rs"]
mod tests;
