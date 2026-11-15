//! Windows-only: key delivery to a hosted application (A26-8). The window
//! listing reports a hosted frame under the hosted application's pid, so
//! `press --app` and `press --window-id` hand that pid to the focus gates;
//! the gates must read the foreground and keyboard focus under the same
//! identity, not under the frame host that owns the frame's handle.

use super::*;
use crate::system::fixture::hosted_settings::{stage_hosted_settings, staged_frame};
use crate::system::fixture::test_support::SHELL_SURFACE_LOCK;
use crate::system::fixture::test_time::deadline;
use crate::tree::fixture::bootstrap;
use agent_desktop_core::{KeyCombo, ProcessIdentity};

fn hosted_identity(pid: u32) -> ProcessIdentity {
    let pid = ProcessId::from(pid);
    let token = process_identity::token_for_pid(pid)
        .expect("the token read answers")
        .expect("a live hosted process carries a generation token");
    ProcessIdentity::new(pid, token)
}

fn shift() -> KeyCombo {
    KeyCombo {
        key: "shift".into(),
        modifiers: vec![],
    }
}

#[test]
fn a_foreground_hosted_application_holds_the_foreground_and_keyboard_under_its_listed_pid() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let staged = stage_hosted_settings();
    let Some(frame) = staged_frame(&staged) else {
        return;
    };
    assert_ne!(
        frame.hosted_pid, frame.frame_host_pid,
        "the staged frame splits its handle's owner from the hosted identity"
    );
    let identity = hosted_identity(frame.hosted_pid);
    assert!(
        process_owns_foreground(identity.pid, &identity.instance)
            .expect("the foreground read answers"),
        "the hosted application owns the foreground its frame holds"
    );
    require_keyboard_focus(identity.pid, &identity.instance, deadline(5_000))
        .expect("the hosted application holds the keyboard focus its frame holds");
}

#[test]
fn press_for_app_delivers_to_a_foreground_hosted_application() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let staged = stage_hosted_settings();
    let Some(frame) = staged_frame(&staged) else {
        return;
    };
    let identity = hosted_identity(frame.hosted_pid);
    synthesis_probe::reset();
    let result = press_for_app_impl(
        identity,
        &shift(),
        InteractionPolicy::headless(),
        deadline(10_000),
    )
    .expect("a foreground hosted application receives the key");
    assert_eq!(result.action, "press_key");
    assert_eq!(
        synthesis_probe::count(),
        1,
        "the key is synthesized exactly once"
    );
}
