//! Windows-only: `close-app` on a hosted application (A26-8) posts `WM_CLOSE`
//! to the application frame host frame the window listing reports under the
//! hosted application's pid, which closes that application, and to no other
//! frame the frame host owns.

use super::*;
use crate::system::close_outcome::CloseOutcome;
use crate::system::fixture::hosted_settings::{
    HOSTED_APP_IMAGE, hosted_process_identity, stage_hosted_settings,
};
use crate::system::fixture::test_support::SHELL_SURFACE_LOCK;
use crate::system::fixture::test_time::deadline;
use crate::system::window::window_identity::live_window_owner;
use crate::tree::fixture::bootstrap;

#[test]
fn close_app_closes_a_hosted_application_through_its_frame() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut staged = stage_hosted_settings();
    let Some((handle, hosted_pid)) = staged
        .frame
        .as_ref()
        .map(|frame| (frame.handle, frame.hosted_pid))
    else {
        return;
    };
    let pid = ProcessId::from(hosted_pid);
    let windows = top_level_windows_for_pid(pid).expect("the window walk answers");
    assert!(
        windows.contains(&handle),
        "the hosted frame is a close target"
    );
    for target in &windows {
        let owner = live_window_owner(*target as _).map(u32::from);
        assert!(
            *target == handle || owner == Some(hosted_pid),
            "{target} is neither the hosted frame nor a window the hosted application owns (owner {owner:?})"
        );
    }
    if !staged.started_settings() {
        eprintln!("skip closing hosted Settings: it was open before the test staged it");
        return;
    }
    let identity = hosted_process_identity(hosted_pid);
    let app = AppInfo {
        name: HOSTED_APP_IMAGE.into(),
        pid,
        bundle_id: None,
        process_instance: Some(identity.instance.clone()),
        presentation: None,
    };
    let outcome = close_app_impl(&app, false, deadline(20_000))
        .expect("a graceful close reaches the hosted application");
    assert_eq!(outcome, CloseOutcome::Exited);
    staged.frame = None;
}
