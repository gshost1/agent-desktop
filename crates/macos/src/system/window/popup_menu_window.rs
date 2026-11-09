//! Reads the window server for an application's on-screen pop-up menu windows.
//!
//! A context menu some toolkits open is absent from the application's
//! accessibility tree, yet the window server still lists it as an on-screen
//! window owned by the application's process at the pop-up menu level. Only
//! the process id and the layer are read, never a title or an owner name.
//! Tooltips, floating panels and other raised windows sit at other levels and
//! are not menus.

use agent_desktop_core::AdapterError;

use super::cg_window::{WindowDictionary, int_field};

/// `kCGPopUpMenuWindowLevel`, the window level the window server gives a menu.
pub(crate) const POPUP_MENU_WINDOW_LEVEL: i64 = 101;

/// Window numbers of the on-screen pop-up menu windows that `pid` owns.
pub(crate) fn menu_window_numbers(pid: i32) -> Result<Vec<i64>, AdapterError> {
    Ok(menu_numbers_from(&on_screen_dictionaries()?, pid))
}

pub(in crate::system) fn menu_numbers_from(
    dictionaries: &[WindowDictionary],
    pid: i32,
) -> Vec<i64> {
    dictionaries
        .iter()
        .filter(|dictionary| int_field(dictionary, "kCGWindowOwnerPID") == Some(i64::from(pid)))
        .filter(|dictionary| {
            int_field(dictionary, "kCGWindowLayer") == Some(POPUP_MENU_WINDOW_LEVEL)
        })
        .filter_map(|dictionary| int_field(dictionary, "kCGWindowNumber"))
        .collect()
}

fn on_screen_dictionaries() -> Result<Vec<WindowDictionary>, AdapterError> {
    use crate::tree::properties::cf_type::borrowed_cf_dictionary;
    use core_graphics::display::CGDisplay;
    use core_graphics::window::{
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
    };

    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let array = CGDisplay::window_list_info(options, None).ok_or_else(|| {
        super::cg_window::inventory_error(
            "CGWindowListCopyWindowInfo returned null instead of an on-screen window list",
        )
    })?;
    array
        .get_all_values()
        .into_iter()
        .map(|raw| {
            borrowed_cf_dictionary(raw as core_foundation::base::CFTypeRef).ok_or_else(|| {
                super::cg_window::inventory_error("CoreGraphics returned a non-dictionary window")
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "popup_menu_window_tests.rs"]
mod tests;
