//! Windows-only: drives the frame-identity seam against a real hosted
//! Settings application (A26-8), staged by `fixture::hosted_settings`.
#![cfg(target_os = "windows")]

use crate::system::fixture::hosted_settings::{
    HOSTED_APP_IMAGE, stage_hosted_settings, staged_frame,
};
use crate::system::fixture::test_support::SHELL_SURFACE_LOCK;
use crate::system::fixture::test_time::deadline;
use crate::system::process_identity::token_for_pid;
use crate::system::window::window_activate::focus_window;
use crate::system::window::window_ops::{is_foreground_window, list_windows_live, parse_handle};
use crate::tree::fixture::bootstrap;
use agent_desktop_core::{Deadline, InteractionLease, ProcessId, WindowFilter};

/// Both halves of the hosted identity, asserted together: the frame's handle as
/// the id, the hosted application's name and pid as the identity, and the
/// pid demonstrably not the frame host's.
#[test]
fn focused_window_reports_the_frame_handle_with_the_hosted_application_identity() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let staged = stage_hosted_settings();
    let Some(frame) = staged_frame(&staged) else {
        return;
    };
    let filter = WindowFilter {
        focused_only: true,
        app: None,
    };
    let focused = list_windows_live(&filter, deadline(15_000)).expect("the listing succeeds");
    let entry = focused
        .first()
        .expect("the staged frame is the desktop's foreground window");

    assert_eq!(
        entry.id,
        format!("w-{}", frame.handle),
        "the reported id is the frame's handle"
    );
    assert_eq!(
        u32::from(entry.pid),
        frame.hosted_pid,
        "the reported pid is the hosted application's process"
    );
    assert_ne!(
        entry.pid,
        ProcessId::from(frame.frame_host_pid),
        "the reported pid is not the frame host's"
    );
    assert!(
        entry.app.eq_ignore_ascii_case(HOSTED_APP_IMAGE),
        "the reported app is the hosted application's image name, got {}",
        entry.app
    );
    assert!(entry.state.is_focused, "the entry is stamped focused");
}

/// The listings agree as an equality: `--app` scoping resolves the hosted application to
/// exactly the identity `focused_window` reports - same pid, same id, the
/// frame's handle.
#[test]
fn list_windows_app_scoping_agrees_with_focused_window_on_a_hosted_application() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let staged = stage_hosted_settings();
    let Some(frame) = staged_frame(&staged) else {
        return;
    };
    let by_app = list_windows_live(
        &WindowFilter {
            focused_only: false,
            app: Some(String::from("SystemSettings")),
        },
        deadline(15_000),
    )
    .expect("the listing succeeds");
    assert!(
        !by_app.is_empty(),
        "--app must resolve the hosted application rather than its frame host"
    );
    let focused = list_windows_live(
        &WindowFilter {
            focused_only: true,
            app: None,
        },
        deadline(15_000),
    )
    .expect("the listing succeeds");
    let focused_entry = focused
        .first()
        .expect("the staged frame is the desktop's foreground window");

    assert_eq!(
        u32::from(focused_entry.pid),
        frame.hosted_pid,
        "focused_window reports the hosted application's pid"
    );
    for window in &by_app {
        assert_eq!(
            u32::from(window.pid),
            u32::from(focused_entry.pid),
            "one visibly focused application receives one pid"
        );
        assert_eq!(window.id, focused_entry.id, "both listings agree on the id");
    }
    assert_eq!(
        parse_handle(&focused_entry.id) as isize,
        frame.handle,
        "the id both commands agree on is the frame's handle"
    );
    assert!(
        by_app.iter().any(|window| window.id == focused_entry.id),
        "the focused identity is among the --app results"
    );
}

/// The reason the listing reports the frame: the identity `focused_window`
/// handed out is accepted by the window-operation path and focuses the
/// frame it names. The frame is already foreground after staging, so the
/// raise short-circuits by design; the call proves the handle/pid split
/// verifies as stored evidence.
#[test]
fn focus_window_succeeds_against_the_identity_focused_window_reported() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let staged = stage_hosted_settings();
    let Some(frame) = staged_frame(&staged) else {
        return;
    };
    let filter = WindowFilter {
        focused_only: true,
        app: None,
    };
    let focused = list_windows_live(&filter, deadline(15_000)).expect("the listing succeeds");
    let entry = focused
        .into_iter()
        .next()
        .expect("the staged frame is the desktop's foreground window");
    assert_eq!(
        parse_handle(&entry.id) as isize,
        frame.handle,
        "the identity carries the frame's handle"
    );

    let lease =
        InteractionLease::guarded(Deadline::after(10_000).expect("deadline"), ()).expect("lease");
    focus_window(&entry, &lease)
        .expect("focus-window succeeds against the identity focused_window returned");
    assert!(
        is_foreground_window(parse_handle(&entry.id)),
        "the frame owns the foreground after the focus"
    );
    assert_eq!(
        u32::from(entry.pid),
        frame.hosted_pid,
        "the identity that focused is the hosted application's"
    );
}

/// The hosted pid is corroborable the way every stored identity is: a live
/// hosted process yields a Windows generation token, which is what the
/// listing stores and what the hosted branch of the ownership predicate
/// re-checks at every point of use.
#[test]
fn a_live_hosted_process_yields_a_generation_token() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let staged = stage_hosted_settings();
    let Some(frame) = staged.frame.as_ref() else {
        return;
    };
    let token = token_for_pid(ProcessId::from(frame.hosted_pid))
        .expect("the token read answers")
        .expect("a live hosted process carries a generation token");
    assert!(
        token.starts_with("windows-proc-v1:"),
        "the token is the Windows generation shape, got {token}"
    );
}
