//! Windows-only: key delivery to a hosted application (A26-8). The window
//! listing reports a hosted frame under the hosted application's pid, so
//! `press --app` and `press --window-id` hand that pid to the focus gates;
//! the gates must read the foreground and keyboard focus under the same
//! identity, not under the frame host that owns the frame's handle.

use super::*;
use crate::adapter::WindowsAdapter;
use crate::system::capture::capture_test_support::HomeIsolation;
use crate::system::fixture::hosted_settings::{
    HOSTED_APP_IMAGE, stage_hosted_settings, staged_frame,
};
use crate::system::fixture::test_support::{SHELL_SURFACE_LOCK, with_interaction_lease_test_lock};
use crate::system::fixture::test_time::deadline;
use crate::system::private_file::WindowsPrivateFile;
use crate::tree::fixture::bootstrap;
use agent_desktop_core::commands::press::{self, PressArgs};
use agent_desktop_core::{CommandContext, KeyCombo, ProcessIdentity};

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

/// The whole command path: `press --app` resolves the hosted application by
/// the name `list-apps` reports and delivers to it, which needs the app
/// inventory and the focus gates to agree on the hosted identity.
#[test]
fn the_press_command_delivers_to_a_foreground_hosted_application_by_name() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let staged = stage_hosted_settings();
    let Some(frame) = staged_frame(&staged) else {
        return;
    };
    let apps = crate::system::app_ops::list_apps_live(deadline(10_000)).expect("apps list");
    assert!(
        apps.iter().any(|app| u32::from(app.pid) == frame.hosted_pid
            && app.name.eq_ignore_ascii_case(HOSTED_APP_IMAGE)),
        "list-apps names the hosted application under its own pid"
    );
    with_interaction_lease_test_lock(|| {
        let _home = HomeIsolation::enter("agent-desktop-hosted-press-home");
        let _ = agent_desktop_core::install_private_file_ops(Box::new(WindowsPrivateFile::new()));
        let result = press::execute(
            PressArgs {
                combo: "shift".into(),
                app: Some(HOSTED_APP_IMAGE.into()),
                window_id: None,
                ref_id: None,
                force: false,
            },
            &WindowsAdapter::new(),
            &CommandContext::default(),
        )
        .expect("press --app reaches the foreground hosted application");
        assert_eq!(result["action"], "press_key", "{result}");
    });
}
