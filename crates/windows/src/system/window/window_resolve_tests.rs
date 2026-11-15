use super::*;
use crate::system::window::window_ops::list_windows_live;
use agent_desktop_core::{WindowFilter, WindowState};

/// A `WindowInfo` naming a handle no live window answers for, so every test
/// below drives the same destroyed-handle shape and differs only in the check
/// it is making. `process_instance` is the parameter because the token's
/// absence is itself one of those checks.
#[cfg(target_os = "windows")]
fn destroyed_window(process_instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "gone".into(),
        app: "none.exe".into(),
        pid: agent_desktop_core::ProcessId::from(1u32),
        process_instance: process_instance.map(str::to_string),
        bounds: None,
        state: WindowState::default(),
    }
}

/// How long the helper below waits for an inventory the desktop held still
/// for, and how long it pauses between attempts.
#[cfg(target_os = "windows")]
const SETTLED_INVENTORY_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(target_os = "windows")]
const SETTLED_INVENTORY_POLL: std::time::Duration = std::time::Duration::from_millis(25);

/// Stages a fixture host and returns the listed window that belongs to it.
///
/// The fixture is returned rather than dropped here: it owns the process whose
/// window the caller is about to resolve or focus, and dropping it would tear
/// that window down before the assertion runs.
#[cfg(target_os = "windows")]
fn listed_fixture_window() -> (crate::tree::fixture::HostedFixture, WindowInfo) {
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture host starts");
    let listed = settled_window_for(agent_desktop_core::ProcessId::from(fixture.process_id()));
    (fixture, listed)
}

/// Takes inventories until one describes a desktop that did not change
/// underneath it, then picks the fixture's window out of that one.
///
/// `list_windows_live` re-verifies every top-level window it assembles and
/// fails the whole inventory when any single one of them is destroyed or
/// retitled between assembly and verification - the mid-listing identity race
/// it exists to catch. This binary generates that race itself: tests running in
/// parallel stand fixture windows up and tear them down throughout the run, so
/// an inventory taken at an arbitrary moment legitimately refuses, and reading
/// that refusal as the outcome of whatever the caller was pinning reports one
/// test's answer for another test's window. Waiting is what separates them.
/// Only the refusal the race produces is waited on; any other code fails the
/// caller immediately rather than being retried until it disappears.
#[cfg(target_os = "windows")]
fn settled_window_for(pid: agent_desktop_core::ProcessId) -> WindowInfo {
    let expiry = std::time::Instant::now() + SETTLED_INVENTORY_BUDGET;
    let mut last = String::from("no settled inventory listed a window of the fixture's process");
    loop {
        match list_windows_live(
            &WindowFilter::default(),
            Deadline::after(5_000).expect("deadline"),
        ) {
            Ok(windows) => {
                if let Some(listed) = windows.into_iter().find(|window| window.pid == pid) {
                    return listed;
                }
            }
            Err(error) => {
                assert_eq!(
                    error.code,
                    ErrorCode::WindowNotFound,
                    "the only refusal a settling inventory may report is the mid-listing identity race, got {error:?}"
                );
                last = error.message.clone();
            }
        }
        assert!(
            std::time::Instant::now() < expiry,
            "the fixture window was not listed within {SETTLED_INVENTORY_BUDGET:?}: {last}"
        );
        std::thread::sleep(SETTLED_INVENTORY_POLL);
    }
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_reconfirms_a_listed_fixture_window() {
    let (_fixture, expected) = listed_fixture_window();
    let resolved =
        resolve_window_strict(&expected, Deadline::after(5_000).unwrap()).expect("resolve");
    assert_eq!(resolved.id, expected.id);
    assert_eq!(resolved.pid, expected.pid);
    assert_eq!(resolved.process_instance, expected.process_instance);
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_rejects_a_destroyed_handle() {
    let win = destroyed_window(Some("windows-proc-v1:0:0"));
    let err = resolve_window_strict(&win, Deadline::after(1_000).unwrap()).unwrap_err();
    assert_eq!(err.code, ErrorCode::WindowNotFound);
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_refuses_an_expired_deadline() {
    let win = destroyed_window(Some("windows-proc-v1:0:0"));
    let deadline = Deadline::after(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let err = resolve_window_strict(&win, deadline).unwrap_err();
    assert_eq!(err.code, ErrorCode::Timeout);
}

fn presented_surface(pid: u32) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Taskbar".into(),
        app: "explorer.exe".into(),
        pid: agent_desktop_core::ProcessId::from(pid),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    }
}

#[test]
fn a_shell_fault_in_the_surface_fallback_stays_window_not_found() {
    let expected = presented_surface(7);
    let fault = AdapterError::internal("UI Automation root read failed");

    let error = presented_or_refused(Err(fault), &expected, "window is gone")
        .expect_err("a fault confirms nothing");

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.message, "window is gone");
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("UI Automation root read failed")),
        "{error:?}"
    );
}

#[test]
fn the_surface_fallback_confirms_only_the_expected_process() {
    let expected = presented_surface(7);

    let foreign = presented_or_refused(Ok(Some(presented_surface(8))), &expected, "gone")
        .expect_err("another process's surface is not the window");
    let absent = presented_or_refused(Ok(None), &expected, "gone").expect_err("no surface");
    let own = presented_or_refused(Ok(Some(presented_surface(7))), &expected, "gone")
        .expect("the expected process's presented surface confirms");

    assert_eq!(foreign.code, ErrorCode::WindowNotFound);
    assert_eq!(absent.code, ErrorCode::WindowNotFound);
    assert_eq!(own.id, expected.id);
}
