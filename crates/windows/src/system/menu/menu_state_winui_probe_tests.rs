//! Diagnostic only: what the menu detector sees for the host's Notepad menu.

use agent_desktop_core::commands::expand;
use agent_desktop_core::commands::helpers::RefArgs;
use agent_desktop_core::commands::snapshot::{self, SnapshotArgs};
use agent_desktop_core::{CommandContext, SnapshotSurface, WindowFilter};

use crate::adapter::WindowsAdapter;
use crate::system::capture::capture_test_support::HomeIsolation;
use crate::system::fixture::test_time::deadline;
use crate::system::private_file::WindowsPrivateFile;
use crate::system::window::window_enum::enumerate_top_level;
use crate::system::window::window_identity::live_window_owner;
use crate::system::window::window_ops::{list_windows_live, window_class_name};
use crate::tree::fixture::bootstrap;

fn find_named(value: &serde_json::Value, role: &str, name: &str) -> Option<String> {
    if value.get("role").and_then(|v| v.as_str()) == Some(role)
        && value.get("name").and_then(|v| v.as_str()) == Some(name)
    {
        if let Some(id) = value.get("ref_id").and_then(|v| v.as_str()) {
            return Some(id.to_owned());
        }
    }
    match value {
        serde_json::Value::Object(map) => map.values().find_map(|v| find_named(v, role, name)),
        serde_json::Value::Array(items) => items.iter().find_map(|v| find_named(v, role, name)),
        _ => None,
    }
}

fn snap(
    window_id: Option<String>,
    surface: SnapshotSurface,
    app: Option<String>,
) -> Result<serde_json::Value, String> {
    snapshot::execute(
        SnapshotArgs {
            app,
            window_id,
            max_depth: 12,
            include_bounds: false,
            interactive_only: true,
            compact: true,
            surface,
            skeleton: false,
            root_ref: None,
            timeout_ms: Some(15_000),
            force_electron_a11y: false,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .map_err(|e| format!("{} {}", e.code(), e))
}

fn windows_of(pid: agent_desktop_core::ProcessId) {
    let _ = enumerate_top_level(|window| {
        if live_window_owner(window.handle) == Some(pid) {
            eprintln!(
                "PROBE window {} class={:?} visible={} cloaked={} tool={}",
                window.handle as isize,
                window_class_name(window.handle),
                window.visible,
                window.cloaked,
                window.tool
            );
        }
        true
    });
}

#[test]
fn probe_the_menu_detector_against_the_hosts_notepad() {
    bootstrap();
    let launched = std::process::Command::new("explorer.exe")
        .arg("shell:AppsFolder\\Microsoft.WindowsNotepad_8wekyb3d8bbwe!App")
        .status();
    eprintln!("PROBE launch via AppsFolder: {launched:?}");
    let _home = HomeIsolation::enter("agent-desktop-winui-menu-probe");
    let _ = agent_desktop_core::install_private_file_ops(Box::new(WindowsPrivateFile::new()));
    let mut notepad = None;
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let all = list_windows_live(
            &WindowFilter {
                focused_only: false,
                app: None,
            },
            deadline(10_000),
        )
        .unwrap_or_default();
        if let Some(window) = all
            .into_iter()
            .find(|w| w.app.to_ascii_lowercase().contains("notepad"))
        {
            notepad = Some(window);
            break;
        }
    }
    let Some(window) = notepad else {
        let all = list_windows_live(
            &WindowFilter {
                focused_only: false,
                app: None,
            },
            deadline(10_000),
        )
        .unwrap_or_default();
        for w in all {
            eprintln!("PROBE listed {} app={} title={:?}", w.id, w.app, w.title);
        }
        let _ = enumerate_top_level(|window| {
            let pid = live_window_owner(window.handle);
            let image = pid.and_then(crate::system::process_identity::process_image_name);
            if image
                .as_deref()
                .is_some_and(|i| i.to_ascii_lowercase().contains("notepad"))
            {
                eprintln!(
                    "PROBE raw notepad window {} class={:?} visible={} cloaked={} tool={}",
                    window.handle as isize,
                    window_class_name(window.handle),
                    window.visible,
                    window.cloaked,
                    window.tool
                );
            }
            true
        });
        eprintln!("PROBE no notepad window");
        return;
    };
    eprintln!(
        "PROBE notepad window {} app={} title={:?} pid={}",
        window.id, window.app, window.title, window.pid
    );
    let image = crate::system::process_identity::process_image_name(window.pid);
    eprintln!("PROBE notepad image {image:?}");
    windows_of(window.pid);
    eprintln!(
        "PROBE before: menu_is_open={:?}",
        super::menu_is_open(window.pid, deadline(10_000)).map_err(|e| e.code)
    );
    let tree = match snap(Some(window.id.clone()), SnapshotSurface::Window, None) {
        Ok(tree) => tree,
        Err(error) => {
            eprintln!("PROBE window snapshot failed: {error}");
            return;
        }
    };
    let file = find_named(&tree["tree"], "menuitem", "File");
    eprintln!("PROBE File ref {file:?}");
    let Some(file) = file else {
        eprintln!("PROBE tree {}", tree["tree"]);
        return;
    };
    let expanded = expand::execute(
        RefArgs {
            ref_id: file,
            timeout_ms: None,
        },
        &WindowsAdapter::new(),
        &CommandContext::default(),
    );
    eprintln!(
        "PROBE expand File: {}",
        match expanded {
            Ok(v) => v.to_string(),
            Err(e) => format!("ERR {} {}", e.code(), e),
        }
    );
    std::thread::sleep(std::time::Duration::from_millis(1500));
    windows_of(window.pid);
    eprintln!(
        "PROBE after: menu_is_open={:?}",
        super::menu_is_open(window.pid, deadline(10_000)).map_err(|e| (e.code, e.message))
    );
    let located = super::locate_menu(window.pid, deadline(10_000));
    eprintln!(
        "PROBE after: locate_menu some={:?}",
        located
            .as_ref()
            .map(|l| l.as_ref().map(|l| l.root_handle()))
            .map_err(|e| e.code.clone())
    );
    match snap(None, SnapshotSurface::Menu, Some(window.app.clone())) {
        Ok(menu) => eprintln!("PROBE menu surface: {}", menu["tree"]),
        Err(error) => eprintln!("PROBE menu surface failed: {error}"),
    }
    match snap(Some(window.id.clone()), SnapshotSurface::Window, None) {
        Ok(after) => eprintln!(
            "PROBE save-as in window tree: {:?}",
            find_named(&after["tree"], "menuitem", "Save as").or_else(|| find_named(
                &after["tree"],
                "menuitem",
                "Save As..."
            ))
        ),
        Err(error) => eprintln!("PROBE window snapshot after failed: {error}"),
    }
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &u32::from(window.pid).to_string()])
        .status();
}
