//! Windows-only: a window id a shell-surface snapshot hands out resolves and
//! focuses like any other window id, so a headed ref action on the taskbar
//! is not refused as `WINDOW_NOT_FOUND`. The taskbar is a tool window the
//! agent inventory leaves out (A26-1) and is up on every desktop this
//! adapter resolves, so these tests need no staging.

use agent_desktop_core::commands::focus;
use agent_desktop_core::commands::helpers::RefArgs;
use agent_desktop_core::commands::open_system_surface::{self, OpenSystemSurfaceArgs};
use agent_desktop_core::commands::snapshot::{self, SnapshotArgs};
use agent_desktop_core::{
    AdapterError, AppError, CommandContext, Deadline, InteractionLease, SnapshotSurface,
};

use super::resolve_window_strict;
use crate::adapter::WindowsAdapter;
use crate::system::capture::capture_test_support::HomeIsolation;
use crate::system::fixture::test_support::{
    SHELL_SURFACE_LOCK, or_skip_shell, stage_foreground, with_interaction_lease_test_lock,
};
use crate::system::fixture::test_time::deadline;
use crate::system::private_file::WindowsPrivateFile;
use crate::system::shell::raise_oracle::{responded_since, witness_desktop};
use crate::system::shell::shell_surface::resolve_surface;
use crate::system::shell::shell_surface_open::close_surface;
use crate::system::window::window_activate::focus_window;
use crate::system::window::window_ops::{is_root_foreground_window, parse_handle};
use crate::tree::fixture::bootstrap;

fn foreground() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    (unsafe { GetForegroundWindow() }) as isize
}

fn first_ref(value: &serde_json::Value) -> Option<String> {
    if let Some(id) = value.get("ref_id").and_then(serde_json::Value::as_str) {
        return Some(id.to_owned());
    }
    match value {
        serde_json::Value::Object(map) => map.values().find_map(first_ref),
        serde_json::Value::Array(items) => items.iter().find_map(first_ref),
        _ => None,
    }
}

fn surface_snapshot(surface: SnapshotSurface) -> serde_json::Value {
    snapshot::execute(
        SnapshotArgs {
            app: None,
            window_id: None,
            max_depth: 6,
            include_bounds: false,
            interactive_only: true,
            compact: true,
            surface,
            skeleton: false,
            root_ref: None,
            timeout_ms: Some(10_000),
            force_electron_a11y: false,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .expect("a presented shell surface answers its snapshot")
}

fn focus_first_ref_headed(snapshot: &serde_json::Value) -> Result<serde_json::Value, AppError> {
    let ref_id = first_ref(&snapshot["tree"]).expect("the surface exposes a ref");
    focus::execute(
        RefArgs {
            ref_id,
            timeout_ms: None,
        },
        &WindowsAdapter::new(),
        &CommandContext::default().with_headed(true),
    )
}

fn adapter_error_of(error: AppError) -> AdapterError {
    match error {
        AppError::Adapter(error) => error,
        error => AdapterError::internal(error.to_string()),
    }
}

/// Whatever the test raised is dismissed on every exit path.
struct CloseOnDrop(SnapshotSurface);

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        let _ = close_surface(self.0, deadline(8_000));
    }
}

#[test]
fn the_taskbar_the_surface_resolver_presents_resolves_strictly() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let taskbar = resolve_surface(SnapshotSurface::Taskbar, deadline(5_000))
        .expect("the desktop is readable")
        .expect("the taskbar is always up");

    let live = resolve_window_strict(&taskbar, deadline(10_000))
        .expect("the taskbar's own id resolves although the inventory leaves it out");

    assert_eq!(live.id, taskbar.id);
    assert_eq!(live.pid, taskbar.pid);
    assert_eq!(live.process_instance, taskbar.process_instance);
}

#[test]
fn a_headed_focus_on_a_taskbar_ref_raises_the_taskbar() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    with_interaction_lease_test_lock(|| {
        let _home = HomeIsolation::enter("agent-desktop-headed-taskbar-home");
        let _ = agent_desktop_core::install_private_file_ops(Box::new(WindowsPrivateFile::new()));
        let previous = foreground();
        let snapshot = surface_snapshot(SnapshotSurface::Taskbar);
        let taskbar = parse_handle(snapshot["window"]["id"].as_str().expect("window id"));

        let focused = focus_first_ref_headed(&snapshot);
        let raised = is_root_foreground_window(taskbar);
        if previous != 0 {
            stage_foreground(previous);
        }

        let focused = focused.expect("a headed ref action on the taskbar focuses its window");
        assert_eq!(focused["action"], "focus", "{focused}");
        assert!(raised, "the taskbar owned the foreground after the focus");
    });
}

/// Start is an immersive surface: the shell presents it above every window
/// while the foreground sits on another process's input window (measured on
/// build 26100: `SearchHost.exe` holds it while the surface is
/// `StartMenuExperienceHost.exe`'s), so its id resolves through the surface
/// resolver and the presented surface counts as focused without a raise.
#[test]
fn the_presented_start_menu_resolves_strictly_and_counts_as_focused() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    with_interaction_lease_test_lock(|| {
        let _home = HomeIsolation::enter("agent-desktop-headed-start-home");
        let _ = agent_desktop_core::install_private_file_ops(Box::new(WindowsPrivateFile::new()));
        let _ = close_surface(SnapshotSurface::StartMenu, deadline(5_000));
        let _cleanup = CloseOnDrop(SnapshotSurface::StartMenu);
        let witness = witness_desktop();
        let opened = or_skip_shell(
            "the command opens the start menu",
            open_system_surface::execute(
                OpenSystemSurfaceArgs {
                    surface: SnapshotSurface::StartMenu,
                },
                &WindowsAdapter::new(),
                &CommandContext::default().with_headed(true),
            )
            .map_err(adapter_error_of),
            || responded_since(&witness),
        );
        if opened.is_none() {
            return;
        }
        let start = resolve_surface(SnapshotSurface::StartMenu, deadline(10_000))
            .expect("the desktop is readable")
            .expect("the opened start menu is presented");

        let live = resolve_window_strict(&start, deadline(10_000))
            .expect("the presented start menu's id resolves");
        let lease = InteractionLease::guarded(Deadline::after(10_000).expect("deadline"), ())
            .expect("lease");
        let focused = focus_window(&live, &lease);

        assert_eq!(live.id, start.id);
        focused.expect("the presented start menu counts as focused");
    });
}
