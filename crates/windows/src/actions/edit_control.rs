//! Writes a classic Win32 edit control's text the way typing and paste do.
//!
//! `ValuePattern.SetValue` on a Win32 `Edit` arrives as `WM_SETTEXT`. An edit
//! sends no `EN_CHANGE` for `WM_SETTEXT` when it is multiline or when it is
//! the edit inside a combo box, so the control's owner never learns that the
//! text changed and the combo box never sends `CBN_EDITCHANGE`. The text reads
//! back as written while the owner keeps acting on its old value: on the
//! Windows 11 Save as dialog the File name field read back the written path
//! for 13 s and Save wrote the dialog's suggested name (windows-11-arm runner,
//! runs 43714909590, 43715575507, 43740201813: `WM_SETTEXT` 0 of 2, the
//! UI Automation write 0 of 2).
//!
//! `EM_REPLACESEL` over the whole text is the path keyboard input and paste
//! take. It sends `EN_UPDATE` and `EN_CHANGE` in every edit style, so the
//! combo box forwards `CBN_EDITCHANGE`, and it marks the text modified. The
//! same dialog saved to the written path 2 of 2 through it (run 43740201813).
//!
//! The rule keys on the window class the control was built from, which
//! `RealGetWindowClassW` reports through any superclass, never on the
//! application: an `Edit`, or a `ComboBox` whose edit child takes the text.

use agent_desktop_core::{AdapterError, Deadline};

/// The step label of a value write delivered through `EM_REPLACESEL`.
pub(crate) const EDIT_LABEL: &str = "EditControl.ReplaceSel";

/// Which window takes a value written to a UI Automation element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditTarget {
    /// The element is not a classic edit, or the edit refuses input.
    NotEdit,
    /// The edit window whose text the write replaces.
    Edit(isize),
}

impl EditTarget {
    /// Classifies a window from the class it was built from and the class of
    /// the edit child it owns, when it owns one.
    pub(crate) fn from_classes(handle: isize, real_class: &str, edit_child: Option<isize>) -> Self {
        if real_class.eq_ignore_ascii_case("Edit") {
            return Self::Edit(handle);
        }
        match edit_child {
            Some(child) if real_class.eq_ignore_ascii_case("ComboBox") => Self::Edit(child),
            _ => Self::NotEdit,
        }
    }

    pub(crate) fn handle(self) -> Option<isize> {
        match self {
            Self::Edit(handle) => Some(handle),
            Self::NotEdit => None,
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{AdapterError, Deadline, EditTarget};
    use crate::tree::element::UIAElement;
    use agent_desktop_core::{DeliverySemantics, ErrorCode};
    use std::ffi::c_void;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        ES_READONLY, FindWindowExW, GWL_STYLE, GetWindowLongW, RealGetWindowClassW,
        SMTO_ABORTIFHUNG, SMTO_ERRORONEXIT, SendMessageTimeoutW,
    };

    const EM_SETSEL: u32 = 0x00B1;
    const EM_REPLACESEL: u32 = 0x00C2;
    const EM_GETLIMITTEXT: u32 = 0x00D5;
    const SEND_TIMEOUT_CAP_MS: u64 = 2_000;

    /// The edit window a value write to `element` should replace the text
    /// of, when the element is a classic edit that accepts input and the
    /// value fits its text limit.
    pub(crate) fn edit_target(element: &UIAElement, value: &str, deadline: Deadline) -> EditTarget {
        let Some(handle) = element_window(element) else {
            return EditTarget::NotEdit;
        };
        let Some(class) = real_class(handle) else {
            return EditTarget::NotEdit;
        };
        let child = combo_edit_child(handle);
        match EditTarget::from_classes(handle, &class, child) {
            EditTarget::Edit(edit) if accepts(edit, value, deadline) => EditTarget::Edit(edit),
            _ => EditTarget::NotEdit,
        }
    }

    /// Replaces the edit's whole text with `value` through `EM_SETSEL` and
    /// `EM_REPLACESEL`. `Ok(false)` means the selection message was never
    /// delivered, so nothing changed. A replace that was sent and not
    /// answered may still land, so it reports a delivered, unverified error.
    pub(crate) fn replace_text(
        handle: isize,
        value: &str,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        if send(handle, EM_SETSEL, 0, -1, deadline).is_none() {
            return Ok(false);
        }
        let text: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        match send(handle, EM_REPLACESEL, 1, text.as_ptr() as isize, deadline) {
            Some(_) => Ok(true),
            None => Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The edit control did not answer the text replacement in time",
            )
            .with_platform_detail(format!(
                "SendMessageTimeoutW(EM_REPLACESEL): {}",
                std::io::Error::last_os_error()
            ))
            .with_disposition(DeliverySemantics::delivered_unverified())),
        }
    }

    fn element_window(element: &UIAElement) -> Option<isize> {
        let handle: isize = element.0.get_native_window_handle().ok()?.into();
        (handle != 0).then_some(handle)
    }

    fn real_class(handle: isize) -> Option<String> {
        let mut buffer = [0u16; 256];
        let length = unsafe {
            RealGetWindowClassW(
                handle as *mut c_void,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        };
        let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
        Some(String::from_utf16_lossy(
            &buffer[..length.min(buffer.len())],
        ))
    }

    fn combo_edit_child(handle: isize) -> Option<isize> {
        let class: Vec<u16> = "Edit".encode_utf16().chain(std::iter::once(0)).collect();
        let child = unsafe {
            FindWindowExW(
                handle as *mut c_void,
                std::ptr::null_mut(),
                class.as_ptr(),
                std::ptr::null(),
            )
        };
        (!child.is_null()).then_some(child as isize)
    }

    fn accepts(handle: isize, value: &str, deadline: Deadline) -> bool {
        let window = handle as *mut c_void;
        let style = unsafe { GetWindowLongW(window, GWL_STYLE) };
        if unsafe { IsWindowEnabled(window) } == 0 || style & ES_READONLY != 0 {
            return false;
        }
        let Some(limit) = send(handle, EM_GETLIMITTEXT, 0, 0, deadline) else {
            return false;
        };
        value.encode_utf16().count() <= limit
    }

    fn send(
        handle: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        deadline: Deadline,
    ) -> Option<usize> {
        let timeout = deadline
            .remaining()
            .as_millis()
            .min(u128::from(SEND_TIMEOUT_CAP_MS))
            .max(1);
        let mut answer: usize = 0;
        let sent = unsafe {
            SendMessageTimeoutW(
                handle as *mut c_void,
                message,
                wparam,
                lparam,
                SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
                u32::try_from(timeout).unwrap_or(u32::MAX),
                &mut answer,
            )
        };
        (sent != 0).then_some(answer)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, EditTarget};
    use crate::tree::element::UIAElement;

    pub(crate) fn edit_target(
        _element: &UIAElement,
        _value: &str,
        _deadline: Deadline,
    ) -> EditTarget {
        EditTarget::NotEdit
    }

    pub(crate) fn replace_text(
        _handle: isize,
        _value: &str,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        Err(AdapterError::not_supported("EditControl.ReplaceSel"))
    }
}

#[allow(unused_imports)]
pub(crate) use imp::{edit_target, replace_text};

#[cfg(test)]
#[path = "edit_control_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "edit_control_live_tests.rs"]
mod live_tests;
