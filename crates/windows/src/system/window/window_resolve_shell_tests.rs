//! Windows-only: a window id a shell-surface snapshot hands out resolves and
//! focuses like any other window id, so a headed ref action on the taskbar
//! is not refused as `WINDOW_NOT_FOUND`. The taskbar is a tool window the
//! agent inventory leaves out (A26-1) and is up on every desktop this
//! adapter resolves, so these tests need no staging.

use agent_desktop_core::commands::focus;
use agent_desktop_core::commands::helpers::RefArgs;
use agent_desktop_core::commands::snapshot::{self, SnapshotArgs};
use agent_desktop_core::{CommandContext, SnapshotSurface};

use super::resolve_window_strict;
use crate::adapter::WindowsAdapter;
use crate::system::capture::capture_test_support::HomeIsolation;
use crate::system::fixture::test_support::{
    SHELL_SURFACE_LOCK, stage_foreground, with_interaction_lease_test_lock,
};
use crate::system::fixture::test_time::deadline;
use crate::system::private_file::WindowsPrivateFile;
use crate::system::shell::shell_surface::resolve_surface;
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

fn taskbar_snapshot() -> serde_json::Value {
    snapshot::execute(
        SnapshotArgs {
            app: None,
            window_id: None,
            max_depth: 6,
            include_bounds: false,
            interactive_only: true,
            compact: true,
            surface: SnapshotSurface::Taskbar,
            skeleton: false,
            root_ref: None,
            timeout_ms: Some(10_000),
            force_electron_a11y: false,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .expect("the taskbar is always up, so its snapshot answers")
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
        let snapshot = taskbar_snapshot();
        let taskbar = parse_handle(snapshot["window"]["id"].as_str().expect("window id"));
        let ref_id = first_ref(&snapshot["tree"]).expect("the taskbar exposes a ref");

        let focused = focus::execute(
            RefArgs {
                ref_id,
                timeout_ms: None,
            },
            &WindowsAdapter::new(),
            &CommandContext::default().with_headed(true),
        );
        let raised = is_root_foreground_window(taskbar);
        if previous != 0 {
            stage_foreground(previous);
        }

        let focused = focused.expect("a headed ref action on the taskbar focuses its window");
        assert_eq!(focused["action"], "focus", "{focused}");
        assert!(raised, "the taskbar owned the foreground after the focus");
    });
}
