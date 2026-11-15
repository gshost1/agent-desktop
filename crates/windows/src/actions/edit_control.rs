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
//! A value holding U+0000, a read-only or disabled edit, a value over the
//! edit's text limit, and an edit that cannot scroll its text (a single-line
//! edit without `ES_AUTOHSCROLL`, a multiline one without `ES_AUTOVSCROLL`)
//! keep `ValuePattern.SetValue`, since `EM_REPLACESEL`, like typing, would cut
//! or refuse them while `WM_SETTEXT` writes the whole value.
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
    use crate::actions::chain::{DeliveryOutcome, after_delivery};
    use crate::actions::edit_pending::{MAX_PENDING_REPLACEMENTS, PendingReplacements};
    use crate::actions::edit_replace::{EditMessage, EditMessenger, EditSendFailure, replace_text};
    use crate::actions::value_write::gated_pattern_value_equals;
    use crate::tree::element::UIAElement;
    use std::ffi::c_void;
    use std::sync::mpsc::sync_channel;
    use std::thread::Builder;
    use std::time::Duration;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, FindWindowExW, GWL_STYLE,
        GetWindowLongW, GetWindowThreadProcessId, RealGetWindowClassW, SMTO_ABORTIFHUNG,
        SMTO_ERRORONEXIT, SendMessageTimeoutW, SendMessageW,
    };

    static PENDING_REPLACEMENTS: PendingReplacements =
        PendingReplacements::new(MAX_PENDING_REPLACEMENTS);

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
        let child = if class.eq_ignore_ascii_case("ComboBox") {
            combo_edit_child(handle)
        } else {
            None
        };
        match EditTarget::from_classes(handle, &class, child) {
            EditTarget::Edit(edit) if accepts(edit, value, deadline) => EditTarget::Edit(edit),
            _ => EditTarget::NotEdit,
        }
    }

    /// Replaces the edit's text and reads the value back through the
    /// element's secure-gated value read.
    pub(crate) fn write_edit(
        element: &UIAElement,
        handle: isize,
        value: &str,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        replace_text(&WindowMessenger, handle, value, deadline)?;
        let verified = gated_pattern_value_equals(element, value).map_err(after_delivery)?;
        Ok(DeliveryOutcome::from_observation(verified))
    }

    struct WindowMessenger;

    impl EditMessenger for WindowMessenger {
        fn send(
            &self,
            handle: isize,
            message: EditMessage,
            deadline: Deadline,
        ) -> Result<(), EditSendFailure> {
            match message {
                EditMessage::SelectAll => send(handle, EM_SETSEL, 0, -1, deadline)
                    .map(|_| ())
                    .ok_or_else(|| EditSendFailure::Unanswered(os_detail())),
                EditMessage::ReplaceSelection(text) => {
                    replace_selection(handle, text, deadline, &PENDING_REPLACEMENTS)
                }
            }
        }
    }

    /// Sends `EM_REPLACESEL` with a text buffer that stays allocated until
    /// nothing can read it any more.
    ///
    /// A window of the calling thread runs its window procedure inside the
    /// send, so the buffer outlives every read of it. Any other window gets
    /// the buffer through a worker thread that owns it and sends with a
    /// blocking `SendMessageW`, which returns only after the edit's thread
    /// has processed the message, or after that thread has exited and the
    /// queue holding the message is gone. Either way no later read of the
    /// buffer exists, so the worker frees it on return. The caller waits for
    /// the worker up to the deadline and reports an unanswered replace when
    /// the wait ends first. Each worker holds a `pool` permit for its whole
    /// life, so the buffers and threads waiting on unresponsive edits never
    /// outnumber the pool's capacity; a replace past it is refused unsent.
    pub(super) fn replace_selection(
        handle: isize,
        text: Vec<u16>,
        deadline: Deadline,
        pool: &'static PendingReplacements,
    ) -> Result<(), EditSendFailure> {
        if window_thread(handle) == unsafe { GetCurrentThreadId() } {
            return send(handle, EM_REPLACESEL, 1, text.as_ptr() as isize, deadline)
                .map(|_| ())
                .ok_or_else(|| EditSendFailure::Unanswered(os_detail()));
        }
        let Some(permit) = pool.try_acquire() else {
            return Err(EditSendFailure::NotSent(format!(
                "{} earlier replacements are still waiting for their edit controls",
                pool.capacity()
            )));
        };
        let (answer, answered) = sync_channel(1);
        let worker = Builder::new()
            .name("agent-desktop-edit-replace".into())
            .spawn(move || {
                let text = text;
                let permit = permit;
                unsafe {
                    SendMessageW(
                        handle as *mut c_void,
                        EM_REPLACESEL,
                        1,
                        text.as_ptr() as isize,
                    )
                };
                drop(text);
                drop(permit);
                let _ = answer.send(());
            });
        if let Err(error) = worker {
            return Err(EditSendFailure::NotSent(format!(
                "the replacement thread did not start: {error}"
            )));
        }
        let wait = send_timeout_ms(deadline);
        answered
            .recv_timeout(Duration::from_millis(u64::from(wait)))
            .map_err(|_| {
                EditSendFailure::Unanswered(format!("SendMessageW unanswered after {wait} ms"))
            })
    }

    fn os_detail() -> String {
        format!("SendMessageTimeoutW: {}", std::io::Error::last_os_error())
    }

    fn window_thread(handle: isize) -> u32 {
        unsafe { GetWindowThreadProcessId(handle as *mut c_void, std::ptr::null_mut()) }
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
        let scrolls = if style & ES_MULTILINE != 0 {
            style & ES_AUTOVSCROLL != 0
        } else {
            style & ES_AUTOHSCROLL != 0
        };
        if value.contains('\0')
            || unsafe { IsWindowEnabled(window) } == 0
            || style & ES_READONLY != 0
            || !scrolls
        {
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
        let mut answer: usize = 0;
        let sent = unsafe {
            SendMessageTimeoutW(
                handle as *mut c_void,
                message,
                wparam,
                lparam,
                SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
                send_timeout_ms(deadline),
                &mut answer,
            )
        };
        (sent != 0).then_some(answer)
    }

    fn send_timeout_ms(deadline: Deadline) -> u32 {
        let timeout = deadline
            .remaining()
            .as_millis()
            .clamp(1, u128::from(SEND_TIMEOUT_CAP_MS));
        u32::try_from(timeout).unwrap_or(u32::MAX)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, EditTarget};
    use crate::actions::chain::DeliveryOutcome;
    use crate::tree::element::UIAElement;

    pub(crate) fn edit_target(
        _element: &UIAElement,
        _value: &str,
        _deadline: Deadline,
    ) -> EditTarget {
        EditTarget::NotEdit
    }

    pub(crate) fn write_edit(
        _element: &UIAElement,
        _handle: isize,
        _value: &str,
        _deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        Err(AdapterError::not_supported("EditControl.ReplaceSel"))
    }
}

#[allow(unused_imports)]
pub(crate) use imp::{edit_target, write_edit};

#[cfg(all(test, target_os = "windows"))]
use imp::replace_selection;

#[cfg(test)]
#[path = "edit_control_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "edit_control_test_support.rs"]
mod test_support;

#[cfg(all(test, target_os = "windows"))]
#[path = "edit_control_live_tests.rs"]
mod live_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "edit_control_pending_live_tests.rs"]
mod pending_live_tests;
