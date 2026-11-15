use agent_desktop_core::ProcessId;

#[cfg(target_os = "windows")]
use super::window_enum::WindowHandle;

/// The top-level window class the shell gives a hosted application's frame
/// (A26-8: the foreground window while a hosted Settings session is active).
#[cfg(target_os = "windows")]
pub(crate) const FRAME_WINDOW_CLASS: &str = "ApplicationFrameWindow";

/// The window class a hosted application's own surface presents inside its
/// host's frame (A26-8: the frame's immediate children split by pid - the
/// title bar and input sink on the frame host's pid, the CoreWindow on the
/// hosted application's).
#[cfg(target_os = "windows")]
const CORE_WINDOW_CLASS: &str = "Windows.UI.Core.CoreWindow";

/// The process a top-level window hosts, when that window is an application
/// frame host frame carrying a live hosted application (A26-8).
///
/// Detection requires both the `ApplicationFrameWindow` class and a child
/// `Windows.UI.Core.CoreWindow` whose owning process differs from the
/// frame's. The class alone is insufficient: this desktop carries
/// `ApplicationFrameWindow` frames owned by the shell with no hosted
/// `CoreWindow` beneath them, and treating those as hosted would attribute
/// a phantom application to the desktop. Suspension drops the hosted
/// `CoreWindow` while its frame survives uncloaked (measured on this
/// build by activating the same single-instanced frame twice), so a
/// suspended application's frame reads as its frame host until the
/// application resumes - and every identity that was verified against the
/// hosted pid fails closed for exactly as long.
///
/// `None` is every kind of "no hosted application here": a different
/// class, an unreadable owner, no `CoreWindow` child, and every
/// `CoreWindow` child owned by the frame's own process.
#[cfg(target_os = "windows")]
pub(crate) fn hosted_application_pid(handle: WindowHandle) -> Option<ProcessId> {
    let frame_pid = super::window_identity::live_window_owner(handle)?;
    hosted_core_window(handle, frame_pid).map(|(_, pid)| pid)
}

/// The hosted application's own `CoreWindow` under an application frame host
/// frame owned by `frame_pid`, with the process that owns it (A26-8). `None`
/// under the same conditions as [`hosted_application_pid`]. The caller passes
/// the frame's owner it already read, so one decision reads it once.
#[cfg(target_os = "windows")]
pub(crate) fn hosted_core_window(
    handle: WindowHandle,
    frame_pid: ProcessId,
) -> Option<(WindowHandle, ProcessId)> {
    if super::window_ops::window_class_name(handle)?.as_str() != FRAME_WINDOW_CLASS {
        return None;
    }
    let core_window = foreign_owned_core_window_child(handle, frame_pid)?;
    Some((
        core_window,
        super::window_identity::live_window_owner(core_window)?,
    ))
}

/// The window whose own thread answers for `pid` behind a top-level window:
/// the window itself when `pid` owns it, or the hosted `CoreWindow` when the
/// window is a frame hosting `pid`'s live application (A26-8).
///
/// A liveness read, such as whether the application stopped pumping
/// messages, uses this rather than [`is_window_of_process`]: the frame's
/// thread is the frame host's, which keeps pumping while the hosted
/// application hangs, so only the `CoreWindow` reports the hosted
/// application's own state.
#[cfg(target_os = "windows")]
pub(crate) fn window_answering_for(handle: WindowHandle, pid: ProcessId) -> Option<WindowHandle> {
    let owner = super::window_identity::live_window_owner(handle)?;
    if owner == pid {
        return Some(handle);
    }
    hosted_core_window(handle, owner)
        .filter(|(_, hosted)| *hosted == pid)
        .map(|(core_window, _)| core_window)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn hosted_application_pid(
    _handle: super::window_enum::WindowHandle,
) -> Option<ProcessId> {
    None
}

/// Whether a window answers to `pid` under the identity the window listing
/// reports: the window's own owner is `pid`, or the window is an application
/// frame host frame hosting `pid`'s live application (A26-8).
///
/// A caller comparing a foreground or keyboard-focus handle against a listed
/// pid uses this, never the raw owner alone: the raw owner of a hosted frame
/// is the frame host, which no listed identity carries while the application
/// is live. Only the frame itself is credited to the hosted application; the
/// frame host's own children, such as the title bar, keep the frame host's
/// identity, so a key aimed at them is never reported as delivered to the
/// hosted application.
#[cfg(target_os = "windows")]
pub(crate) fn is_window_of_process(handle: WindowHandle, pid: ProcessId) -> bool {
    match super::window_identity::live_window_owner(handle) {
        Some(owner) if owner == pid => true,
        Some(owner) => hosted_core_window(handle, owner).map(|(_, hosted)| hosted) == Some(pid),
        None => false,
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_window_of_process(
    _handle: super::window_enum::WindowHandle,
    _pid: ProcessId,
) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn window_answering_for(
    _handle: super::window_enum::WindowHandle,
    _pid: ProcessId,
) -> Option<super::window_enum::WindowHandle> {
    None
}

/// The first child `CoreWindow` whose owning process differs from the
/// frame's, walked in z-order through the frame's immediate children. A
/// child whose owner cannot be read is passed over, not fatal: one raced
/// child must not fail the walk while a later sibling could still answer.
#[cfg(target_os = "windows")]
fn foreign_owned_core_window_child(
    frame: WindowHandle,
    frame_pid: ProcessId,
) -> Option<WindowHandle> {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowExW;

    let class = wide(CORE_WINDOW_CLASS);
    let mut after = std::ptr::null_mut();
    loop {
        let child = unsafe { FindWindowExW(frame, after, class.as_ptr(), std::ptr::null()) };
        if child.is_null() {
            return None;
        }
        match super::window_identity::live_window_owner(child) {
            Some(pid) if pid != frame_pid => return Some(child),
            _ => after = child,
        }
    }
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
#[path = "frame_identity_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "frame_identity_settings_tests.rs"]
mod settings_tests;
