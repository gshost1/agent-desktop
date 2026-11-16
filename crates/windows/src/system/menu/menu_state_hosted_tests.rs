//! Windows-only: the tool-window menu pool for a hosted application (A26-8).
//! The hosted identity credits a hosted application with exactly one window
//! beyond those it owns, its frame, and a frame is neither a tool window nor a
//! popup, so the pool reads raw ownership and loses nothing: this pins that
//! the frame and every frame host window stay out of it.

use super::*;
use crate::system::fixture::hosted_settings::stage_hosted_settings;
use crate::system::fixture::test_support::SHELL_SURFACE_LOCK;
use crate::tree::fixture::bootstrap;

#[test]
fn a_hosted_application_menu_pool_holds_no_frame_host_window() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let staged = stage_hosted_settings();
    let Some(frame) = staged.frame.as_ref() else {
        return;
    };
    let pid = ProcessId::from(frame.hosted_pid);
    let frame_host = ProcessId::from(frame.frame_host_pid);
    let candidates = tool_window_candidates(pid).expect("the window walk answers");
    for handle in &candidates {
        let handle = *handle as crate::system::window::window_enum::WindowHandle;
        assert_eq!(
            crate::system::window::window_identity::live_window_owner(handle),
            Some(pid)
        );
        assert_ne!(
            crate::system::window::window_identity::live_window_owner(handle),
            Some(frame_host),
            "a frame host window is never a hosted application's menu candidate"
        );
    }
    assert!(
        !candidates.contains(&frame.handle),
        "the frame is a window, not a menu candidate"
    );
}
