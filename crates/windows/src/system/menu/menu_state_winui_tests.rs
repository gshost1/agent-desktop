//! Windows-only: a WinUI 3 menu flyout reads as an open menu. The test
//! drives the Store Notepad, which Windows 11 carries and the hosted
//! Windows 11 runner has (Microsoft.WindowsNotepad 11.2605 on build 26200);
//! a host without the package launches nothing and the test says so. A host
//! with the package that shows no new Notepad window fails the test.

use agent_desktop_core::commands::helpers::RefArgs;
use agent_desktop_core::commands::snapshot::{self, SnapshotArgs};
use agent_desktop_core::commands::{collapse, expand};
use agent_desktop_core::{CommandContext, ProcessId, SnapshotSurface, WindowFilter, WindowInfo};

use crate::adapter::WindowsAdapter;
use crate::system::capture::capture_test_support::HomeIsolation;
use crate::system::fixture::test_support::SHELL_SURFACE_LOCK;
use crate::system::fixture::test_time::deadline;
use crate::system::private_file::WindowsPrivateFile;
use crate::system::window::window_ops::list_windows_live;
use crate::tree::fixture::bootstrap;

const STORE_NOTEPAD_APP_ID: &str = "shell:AppsFolder\\Microsoft.WindowsNotepad_8wekyb3d8bbwe!App";

/// The launched Store Notepad, killed on drop.
struct LaunchedNotepad(WindowInfo);

impl Drop for LaunchedNotepad {
    fn drop(&mut self) {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &u32::from(self.0.pid).to_string()])
            .status();
    }
}

fn notepad_windows() -> Vec<WindowInfo> {
    list_windows_live(
        &WindowFilter {
            focused_only: false,
            app: None,
        },
        deadline(10_000),
    )
    .unwrap_or_default()
    .into_iter()
    .filter(|window| window.app.eq_ignore_ascii_case("Notepad.exe"))
    .collect()
}

fn store_notepad_installed() -> bool {
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "if (Get-AppxPackage -Name Microsoft.WindowsNotepad) { exit 0 } else { exit 1 }",
        ])
        .status()
        .is_ok_and(|status| status.success())
}

fn launch_store_notepad() -> Option<LaunchedNotepad> {
    if !store_notepad_installed() {
        return None;
    }
    let before: Vec<ProcessId> = notepad_windows().iter().map(|window| window.pid).collect();
    let _ = std::process::Command::new("explorer.exe")
        .arg(STORE_NOTEPAD_APP_ID)
        .status();
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if let Some(window) = notepad_windows()
            .into_iter()
            .find(|window| !before.contains(&window.pid))
        {
            return Some(LaunchedNotepad(window));
        }
    }
    panic!(
        "the Store Notepad package is installed but no new Notepad process showed a window \
         within 30 s; a running Notepad may have taken the launch"
    );
}

fn find_ref(value: &serde_json::Value, role: &str, name: &str) -> Option<String> {
    if value.get("role").and_then(serde_json::Value::as_str) == Some(role)
        && value.get("name").and_then(serde_json::Value::as_str) == Some(name)
    {
        if let Some(id) = value.get("ref_id").and_then(serde_json::Value::as_str) {
            return Some(id.to_owned());
        }
    }
    match value {
        serde_json::Value::Object(map) => map.values().find_map(|v| find_ref(v, role, name)),
        serde_json::Value::Array(items) => items.iter().find_map(|v| find_ref(v, role, name)),
        _ => None,
    }
}

fn file_menu_ref(window: &WindowInfo) -> String {
    let tree = snapshot::execute(
        SnapshotArgs {
            app: None,
            window_id: Some(window.id.clone()),
            max_depth: 12,
            include_bounds: false,
            interactive_only: true,
            compact: true,
            surface: SnapshotSurface::Window,
            skeleton: false,
            root_ref: None,
            timeout_ms: Some(15_000),
            force_electron_a11y: false,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .expect("the Notepad window snapshots");
    find_ref(&tree["tree"], "menuitem", "File").expect("Notepad exposes its File menu item")
}

fn wait_for_menu_state(pid: ProcessId, open: bool) -> bool {
    for _ in 0..40 {
        if super::menu_is_open(pid, deadline(10_000)).expect("the detector answers") == open {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

#[test]
fn a_winui_menu_flyout_reads_as_an_open_menu_and_closes_again() {
    bootstrap();
    let _shell = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(notepad) = launch_store_notepad() else {
        eprintln!("skip winui menu: the Store Notepad package is not installed");
        return;
    };
    let _home = HomeIsolation::enter("agent-desktop-winui-menu-home");
    let _ = agent_desktop_core::install_private_file_ops(Box::new(WindowsPrivateFile::new()));
    let pid = notepad.0.pid;
    let file = file_menu_ref(&notepad.0);
    assert!(
        wait_for_menu_state(pid, false),
        "no menu reads as open at rest"
    );

    expand::execute(
        RefArgs {
            ref_id: file.clone(),
            timeout_ms: None,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .expect("the File menu expands");

    assert!(
        wait_for_menu_state(pid, true),
        "the open File flyout reads as an open menu"
    );
    assert!(
        super::locate_menu(pid, deadline(10_000))
            .expect("the locator answers")
            .is_some(),
        "the menu surface can root at the open flyout"
    );
    let menu = snapshot::execute(
        SnapshotArgs {
            app: Some(notepad.0.app.clone()),
            window_id: None,
            max_depth: 12,
            include_bounds: false,
            interactive_only: true,
            compact: true,
            surface: SnapshotSurface::Menu,
            skeleton: false,
            root_ref: None,
            timeout_ms: Some(15_000),
            force_electron_a11y: false,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .expect("the menu surface roots at the open flyout");
    let names: Vec<String> = menu["tree"]["children"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|child| child["name"].as_str().map(str::to_owned))
        .collect();
    eprintln!("menu surface direct children: {names:?}");
    assert!(
        find_ref(&menu["tree"], "menuitem", "Save as").is_some(),
        "the menu surface holds the flyout's items: {}",
        menu["tree"]
    );
    collapse::execute(
        RefArgs {
            ref_id: file,
            timeout_ms: None,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .expect("the File menu collapses");
    assert!(
        wait_for_menu_state(pid, false),
        "the closed File flyout no longer reads as an open menu"
    );
}
