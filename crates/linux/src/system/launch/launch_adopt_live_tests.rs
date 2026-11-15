use super::*;

const FORWARDING_APP: &str = "import sys, gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, Gio
app = Gtk.Application(application_id='org.adl.AdoptLive', flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE)
shown = []
def command_line(app, line):
    pages = line.get_arguments()[1:]
    if not shown:
        shown.append(Gtk.ApplicationWindow(application=app, title='adl-adopt'))
        shown[0].show_all()
    else:
        shown[0].set_title('adl-adopt ' + ' '.join(pages))
    shown[0].present()
    return 0
app.connect('command-line', command_line)
sys.exit(app.run(sys.argv))
";

fn title_of(window: &str) -> String {
    xprop(&["-id", window, "_NET_WM_NAME"])
}

#[test]
#[ignore = "drives real applications; run through scripts/private-session.sh --wm"]
fn live_a_forward_that_only_changes_the_running_window_adopts_that_instance() {
    let dir = std::env::var("AGENT_DESKTOP_PRIVATE_SESSION").expect("a private session");
    let script = format!("{dir}/adl-adopt.py");
    std::fs::write(&script, FORWARDING_APP).expect("a script");
    let mut owned = Owned(Vec::new());
    let first = launch("/usr/bin/python3", &options(&[&script])).expect("the first launch");
    let pid = owned.keep(first.pid.get());
    let window = first.window.expect("the first window");
    wait_until("the native window", || x_windows_of(pid).len() == 1);
    let native = x_windows_of(pid);
    let started = Instant::now();
    let forwarded = launch("/usr/bin/python3", &options(&[&script, "page2"]))
        .expect("a forward into the running window is a launch");
    let waited = started.elapsed();
    assert_eq!(forwarded.pid.get(), pid, "{forwarded:?}");
    assert_eq!(forwarded.window.map(|shown| shown.id), Some(window.id));
    let note = forwarded.notes.suggestion.unwrap_or_default();
    assert!(
        note.contains(&format!("running instance (pid {pid})")),
        "{note}"
    );
    assert!(waited < Duration::from_secs(10), "waited {waited:?}");
    wait_until("the forwarded page", || {
        title_of(&native[0]).contains("page2")
    });
    assert_eq!(x_windows_of(pid), native, "no new window opened");
    println!(
        "independent xprop: {} after {waited:?}",
        title_of(&native[0])
    );
    let unrelated = launch(
        "/usr/bin/python3",
        &LaunchOptions {
            timeout_ms: 6_000,
            ..options(&["-c", "pass"])
        },
    )
    .expect_err("another script that exits cleanly is not a handoff");
    assert_eq!(unrelated.code, ErrorCode::ActionFailed, "{unrelated:?}");
}
