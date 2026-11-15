//! Windows-only: `list-surfaces` for a hosted application (A26-8) lists the
//! application frame host frame the window listing reports under the hosted
//! application's pid, and no other window the frame host owns.

use super::*;
use crate::system::fixture::hosted_settings::{hosted_process_identity, stage_hosted_settings};
use crate::system::fixture::test_support::SHELL_SURFACE_LOCK;
use crate::system::fixture::test_time::deadline;
use crate::system::window::window_identity::live_window_owner;
use crate::tree::fixture::bootstrap;

#[test]
fn a_hosted_application_lists_its_frame_as_its_window_surface() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let staged = stage_hosted_settings();
    let Some(frame) = staged.frame.as_ref() else {
        return;
    };
    let surfaces =
        list_surfaces_for_process(hosted_process_identity(frame.hosted_pid), deadline(15_000))
            .expect("the hosted application's surfaces list");
    let frame_id = format!("w-{}", frame.handle);
    assert!(
        surfaces
            .iter()
            .any(|surface| surface.kind == "window" && surface.id == frame_id),
        "the frame is the hosted application's window surface: {surfaces:?}"
    );
    for surface in surfaces.iter().filter(|surface| surface.kind == "window") {
        let handle = surface
            .id
            .trim_start_matches("w-")
            .parse::<usize>()
            .expect("a window surface id carries a handle") as WindowHandle;
        let owner = live_window_owner(handle).map(u32::from);
        assert!(
            surface.id == frame_id || owner == Some(frame.hosted_pid),
            "{} is neither the hosted frame nor a window the hosted application owns (owner {owner:?})",
            surface.id
        );
    }
}
