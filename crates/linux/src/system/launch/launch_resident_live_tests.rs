use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use agent_desktop_core::system::launch_options::LaunchOptions;

use super::*;
use agent_desktop_core::system::launch_presentation::LaunchPresentation;

const RESIDENT_APP: &str = "#!/usr/bin/env python3
import os, gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, GLib
Gtk.init()
if os.environ.get('ADL_MODE') == 'none':
    Gtk.main()
w = Gtk.Window(title='adl-resident')
w.set_default_size(200, 100)
w.show_all()
if os.environ.get('ADL_MODE') == 'resident':
    def hide_when_requested():
        if not os.path.exists(os.environ['ADL_HIDE']):
            return True
        w.destroy()
        return False
    GLib.timeout_add(20, hide_when_requested)
    GLib.timeout_add(60000, Gtk.main_quit)
else:
    w.connect('destroy', Gtk.main_quit)
Gtk.main()
";

#[test]
#[ignore = "drives real applications; run through scripts/private-session.sh --wm"]
fn live_a_running_app_with_no_window_is_started_again_and_a_window_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::var("AGENT_DESKTOP_PRIVATE_SESSION").expect("a private session");
    let script = format!("{dir}/adl-resident");
    std::fs::write(&script, RESIDENT_APP).expect("a script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("modes");
    let hide = format!("{dir}/hide-resident");
    let resident = crate::system::close::close_live_tests::rig::Orphan::spawn(
        &script,
        &[],
        &[
            ("ADL_MODE", "resident".to_owned()),
            ("ADL_HIDE", hide.clone()),
        ],
    );
    resident.wait_for_window();
    let resident_pid = resident.pid;
    std::fs::write(hide, "hide").expect("request window destruction");
    wait_until("the resident window to leave the display", || {
        x_windows_of(resident_pid).is_empty()
    });
    wait_for_no_listed_window(resident_pid);
    assert!(
        process_facts::read(resident_pid).is_some(),
        "the process stays"
    );

    let normal = LaunchOptions {
        presentation: LaunchPresentation {
            activate: true,
            ..Default::default()
        },
        env: BTreeMap::from([("ADL_MODE".to_owned(), "normal".to_owned())]),
        ..options(&[])
    };
    let second = launch(&script, &normal).expect("a launch of an app with no window");
    let mut owned = Owned(Vec::new());
    owned.keep(second.pid.get());
    assert!(second.window.is_some(), "a window is reported: {second:?}");
    assert_ne!(
        second.pid.get(),
        resident_pid,
        "the app that shows the window is reported, not the one with none"
    );
    assert!(!x_windows_of(second.pid.get()).is_empty());
}

fn windows_of_pid(pid: u32) -> Result<usize, AdapterError> {
    crate::system::list_windows(
        &SharedBus::default(),
        &agent_desktop_core::WindowFilter::default(),
        deadline(8_000),
    )
    .map(|windows| windows.iter().filter(|w| w.pid.get() == pid).count())
}

fn wait_for_no_listed_window(pid: u32) {
    let started = Instant::now();
    loop {
        let read = windows_of_pid(pid);
        if matches!(read, Ok(0)) {
            return;
        }
        assert!(
            started.elapsed() < SETTLE,
            "the listing never showed pid {pid} without a window: {read:?}"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
#[ignore = "drives real applications; run through scripts/private-session.sh --wm"]
fn live_a_forwarded_launch_without_matching_argv_reports_the_residents_new_window() {
    let mut owned = Owned(Vec::new());
    let first = launch("nautilus", &options(&[])).expect("the first launch");
    let pid = owned.keep(first.pid.get());
    wait_until("the initial native window", || x_windows_of(pid).len() == 1);
    let before = x_windows_of(pid);
    assert_eq!(before.len(), 1);
    let (known, _) = crate::system::window::window_ops::windows_of_process(
        &SharedBus::default(),
        pid,
        deadline(8_000),
    )
    .expect("the resident's original windows");
    let mut forwarded = options(&["--new-window"]);
    forwarded.presentation.activate = true;
    let result = launch("nautilus", &forwarded).expect("a window proves the forwarded owner");
    assert_eq!(result.pid.get(), pid);
    let window = result.window.expect("the new window is reported");
    assert!(!known.iter().any(|old| old.id == window.id));
    wait_until("native forwarding effect", || {
        x_windows_of(pid).len() > before.len()
    });
    let after = x_windows_of(pid);
    assert!(before.iter().all(|id| after.contains(id)));
    println!("independent xprop: pid {pid} opened another window after forwarding");
}

#[test]
#[ignore = "drives real applications; run through scripts/private-session.sh --wm"]
fn live_a_launch_that_shows_no_window_does_not_wait_for_the_whole_timeout() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::var("AGENT_DESKTOP_PRIVATE_SESSION").expect("a private session");
    let script = format!("{dir}/adl-resident");
    std::fs::write(&script, RESIDENT_APP).expect("a script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("modes");
    let mut owned = Owned(Vec::new());
    let options = LaunchOptions {
        env: BTreeMap::from([("ADL_MODE".to_owned(), "none".to_owned())]),
        ..options(&[])
    };
    for round in 0..2 {
        let started = Instant::now();
        let result = launch(&script, &options).expect("a launch");
        owned.keep(result.pid.get());
        assert!(result.window.is_none(), "round {round}: {result:?}");
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "round {round} waited {:?} of a 15 s timeout",
            started.elapsed()
        );
    }
}

fn launch_off_bus(id: &str, options: &LaunchOptions) -> (LaunchResult, Duration) {
    let started = Instant::now();
    let result = launch(id, options).expect("a launch");
    let waited = started.elapsed();
    assert!(result.window.is_none(), "{result:?}");
    assert!(process_facts::read(result.pid.get()).is_some());
    (result, waited)
}

fn suggestion_of(result: &LaunchResult) -> &str {
    result.notes.suggestion.as_deref().unwrap_or_default()
}

#[test]
#[ignore = "drives real applications; run through scripts/private-session.sh --wm"]
fn live_a_process_that_will_never_join_the_bus_is_reported_after_the_grace_not_the_timeout() {
    let mut owned = Owned(Vec::new());
    let windowed = LaunchOptions {
        timeout_ms: 20_000,
        ..options(&[])
    };
    let (result, waited) = launch_off_bus("xlogo", &windowed);
    owned.keep(result.pid.get());
    assert!(
        suggestion_of(&result).contains("shows a window"),
        "{result:?}"
    );
    assert!(waited < Duration::from_secs(9), "waited {waited:?} of 20 s");
    let bridge_off = LaunchOptions {
        timeout_ms: 20_000,
        env: BTreeMap::from([("NO_AT_BRIDGE".to_owned(), "1".to_owned())]),
        ..options(&["120"])
    };
    let (result, waited) = launch_off_bus("/bin/sleep", &bridge_off);
    owned.keep(result.pid.get());
    assert!(
        suggestion_of(&result).contains("NO_AT_BRIDGE"),
        "{result:?}"
    );
    assert!(waited < Duration::from_secs(9), "waited {waited:?} of 20 s");
}

#[test]
#[ignore = "drives real applications; run through scripts/private-session.sh --wm"]
fn live_a_process_with_no_window_yet_is_waited_for_until_the_timeout() {
    let mut owned = Owned(Vec::new());
    let starting = LaunchOptions {
        timeout_ms: 8_000,
        ..options(&["120"])
    };
    let (result, waited) = launch_off_bus("/bin/sleep", &starting);
    owned.keep(result.pid.get());
    assert!(
        suggestion_of(&result).contains("still starting"),
        "{result:?}"
    );
    assert!(!suggestion_of(&result).contains("toolkit"), "{result:?}");
    assert!(
        waited >= Duration::from_millis(7_500),
        "waited {waited:?} of 8 s"
    );
    assert!(waited < Duration::from_secs(9), "waited {waited:?} of 8 s");
}

#[test]
#[ignore = "drives real applications; run through scripts/private-session.sh --wm"]
fn live_a_process_started_through_an_exec_wrapper_entry_answers_to_that_entry() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::var("AGENT_DESKTOP_PRIVATE_SESSION").expect("a private session");
    let data = std::env::var("XDG_DATA_HOME").expect("private data");
    let app = format!("{dir}/adl-wrapped-app");
    let wrapper = format!("{dir}/adl-wrapper");
    std::fs::write(&app, RESIDENT_APP).expect("an app");
    std::fs::write(
        &wrapper,
        format!("#!/bin/bash\nexec -a \"$0\" /usr/bin/python3 {app} \"$@\"\n"),
    )
    .expect("a wrapper");
    for file in [&app, &wrapper] {
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).expect("modes");
    }
    let entries = std::path::Path::new(&data).join("applications");
    std::fs::create_dir_all(&entries).expect("entries");
    std::fs::write(
        entries.join("adl-wrapped.desktop"),
        format!("[Desktop Entry]\nType=Application\nName=Adl Wrapped\nExec={wrapper} %U\n"),
    )
    .expect("an entry");
    let mut owned = Owned(Vec::new());
    let result = launch("adl-wrapped", &options(&["extra-argument"])).expect("a launch");
    let pid = owned.keep(result.pid.get());
    let facts = process_facts::read(pid).expect("the launched process");
    let aliases = crate::system::app::app_name::aliases_of(pid, &facts);
    assert!(
        aliases.iter().any(|alias| alias == "Adl Wrapped"),
        "{aliases:?} for {facts:?}"
    );
    assert!(
        aliases.iter().any(|alias| alias == "adl-wrapped"),
        "{aliases:?}"
    );
    assert_eq!(
        crate::system::app::app_name::desktop_id(pid, &facts).as_deref(),
        Some("adl-wrapped")
    );
}
