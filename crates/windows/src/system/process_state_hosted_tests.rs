//! Windows-only: the hang probe for a hosted application (A26-8) reads the
//! hosted `CoreWindow`, whose thread is the application's, never the frame,
//! whose thread is the frame host's.

use super::*;
use crate::system::fixture::hosted_settings::{hosted_process_identity, stage_hosted_settings};
use crate::system::fixture::test_support::SHELL_SURFACE_LOCK;
use crate::system::fixture::test_time::deadline;
use crate::system::window::window_identity::live_window_owner;
use crate::tree::fixture::bootstrap;

#[test]
fn a_hosted_application_is_probed_through_its_own_core_window() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let staged = stage_hosted_settings();
    let Some(frame) = staged.frame.as_ref() else {
        return;
    };
    let pid = agent_desktop_core::ProcessId::from(frame.hosted_pid);
    let probed = top_level_windows_for(pid).expect("the window walk answers");
    assert!(
        !probed.is_empty(),
        "the hosted application has a window to probe"
    );
    assert!(
        !probed.contains(&frame.handle),
        "the frame's thread belongs to the frame host and never answers for the hosted application"
    );
    let core_window = hosted_core_window_child(frame.handle);
    assert!(
        probed.contains(&core_window),
        "the hosted CoreWindow under the frame is probed: {probed:?} lacks {core_window}"
    );
    for handle in &probed {
        assert_eq!(
            live_window_owner(*handle as _).map(u32::from),
            Some(frame.hosted_pid),
            "every probed window runs on the hosted application's own thread"
        );
    }
    let state = process_state_impl(hosted_process_identity(frame.hosted_pid), deadline(15_000))
        .expect("the hosted application's state reads");
    assert_eq!(state, ProcessState::Running);
}

fn hosted_core_window_child(frame: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowExW;
    let class: Vec<u16> = "Windows.UI.Core.CoreWindow"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let child = unsafe {
        FindWindowExW(
            frame as _,
            std::ptr::null_mut(),
            class.as_ptr(),
            std::ptr::null(),
        )
    };
    assert!(!child.is_null(), "the staged frame hosts a CoreWindow");
    child as isize
}
