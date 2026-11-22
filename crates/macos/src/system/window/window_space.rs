//! Which Mission Control Spaces hold a CoreGraphics window, read through the
//! window server's `CGSCopySpacesForWindows`.
//!
//! A window on another Space, or on the desktop behind a full-screen Space,
//! drops out of its application's `AXWindows` while CoreGraphics still lists
//! it off screen, and it still belongs to its Space. A window its application
//! ordered out belongs to no Space. The symbol is private, so it is resolved at
//! run time and its absence reads as unknown.

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::number::CFNumber;

type MainConnection = unsafe extern "C" fn() -> u32;
type CopySpacesForWindows = unsafe extern "C" fn(u32, i32, CFArrayRef) -> CFArrayRef;

const ALL_SPACES_MASK: i32 = 7;

/// Whether the window server places `window_number` on any Space, or `None`
/// when the Space read is unavailable.
pub(crate) fn is_on_any_space(window_number: i64) -> Option<bool> {
    let (main_connection, copy_spaces) = space_functions()?;
    let requested = CFArray::from_CFTypes(&[CFNumber::from(window_number)]);
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
