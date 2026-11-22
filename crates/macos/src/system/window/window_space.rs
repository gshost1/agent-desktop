//! Which Mission Control Spaces hold a CoreGraphics window, read through the
//! window server's `CGSCopySpacesForWindows`.
//!
//! A window on another Space, or on the desktop behind a full-screen Space,
//! drops out of its application's `AXWindows` while CoreGraphics still lists
//! it off screen, and it still belongs to its Space. A window its application
//! ordered out belongs to no Space. The symbol is private, so it is resolved at
//! run time and its absence reads as unknown.
//!
//! The call is a synchronous window-server round trip with no timeout of its
//! own, so it runs on a worker thread and the caller waits only until its
//! deadline. A read still blocked from an earlier call makes the next read
//! unknown instead of starting a second worker.

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::number::CFNumber;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

type MainConnection = unsafe extern "C" fn() -> u32;
type CopySpacesForWindows = unsafe extern "C" fn(u32, i32, CFArrayRef) -> CFArrayRef;

const ALL_SPACES_MASK: i32 = 7;

static READ_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Whether the window server places `window_number` on any Space, or `None`
/// when the Space read is unavailable or does not answer before `deadline`.
pub(crate) fn is_on_any_space(window_number: i64, deadline: Instant) -> Option<bool> {
    let window_id = u32::try_from(window_number).ok()?;
    let functions = space_functions()?;
    read_before_deadline(&READ_IN_FLIGHT, deadline, move || {
        copy_space_membership(functions, window_id)
    })
}

fn read_before_deadline(
    in_flight: &'static AtomicBool,
    deadline: Instant,
    read: impl FnOnce() -> Option<bool> + Send + 'static,
) -> Option<bool> {
    if Instant::now() >= deadline || in_flight.swap(true, Ordering::AcqRel) {
        return None;
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("agent-desktop-window-space".into())
        .spawn(move || {
            let membership = read();
            in_flight.store(false, Ordering::Release);
            let _ = sender.send(membership);
        });
    if spawned.is_err() {
        in_flight.store(false, Ordering::Release);
        return None;
    }
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok()
        .flatten()
}

fn copy_space_membership(
    (main_connection, copy_spaces): (MainConnection, CopySpacesForWindows),
    window_id: u32,
) -> Option<bool> {
    let window_id = CFNumber::from(i32::from_ne_bytes(window_id.to_ne_bytes()));
    let requested = CFArray::from_CFTypes(&[window_id]);
    let spaces = unsafe {
        let connection = main_connection();
        copy_spaces(connection, ALL_SPACES_MASK, requested.as_concrete_TypeRef())
    };
    if spaces.is_null() {
        return None;
    }
    let spaces = unsafe { CFArray::<CFType>::wrap_under_create_rule(spaces) };
    Some(!spaces.is_empty())
}

fn space_functions() -> Option<(MainConnection, CopySpacesForWindows)> {
    static FUNCTIONS: std::sync::OnceLock<Option<(MainConnection, CopySpacesForWindows)>> =
        std::sync::OnceLock::new();
    *FUNCTIONS.get_or_init(|| {
        let connection =
            unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"CGSMainConnectionID".as_ptr()) };
        let spaces =
            unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"CGSCopySpacesForWindows".as_ptr()) };
        (!connection.is_null() && !spaces.is_null()).then(|| unsafe {
            (
                std::mem::transmute::<*mut std::ffi::c_void, MainConnection>(connection),
                std::mem::transmute::<*mut std::ffi::c_void, CopySpacesForWindows>(spaces),
            )
        })
    })
}

#[cfg(test)]
#[path = "window_space_tests.rs"]
mod tests;
