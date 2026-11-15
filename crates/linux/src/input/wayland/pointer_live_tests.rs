use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use agent_desktop_core::{
    Deadline, DeliverySemantics, DisplayInfo, DragParams, ErrorCode, InputOps, InteractionLease,
    Modifier, MouseButton, MouseEvent, MouseEventKind, Point,
};
use serde_json::Value;

use crate::tree::live::live_launch::Launched;

const FILTER: &str = "input::wayland::pointer_send::live_tests::coordinate_pointer_input_lands_where_the_probe_windows_say";
const MONITORS: &str = "1280x800 1600x1200";
const LAYOUT: &str = r#"
import sys
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
name = path = iface = 'org.gnome.Mutter.DisplayConfig'
path = '/org/gnome/Mutter/DisplayConfig'
def call(method, params=None):
    return bus.call_sync(name, path, iface, method, params, None, Gio.DBusCallFlags.NONE, 5000, None).unpack()
serial, monitors, _, _ = call('GetCurrentState')
mode = {spec[0]: next(m[0] for m in modes if m[6].get('is-current')) for spec, modes, _ in monitors}
layout = [(0, 0, 2.0, 0, True, [('Meta-1', mode['Meta-1'], {})]),
          (800, 0, 1.0, 0, False, [('Meta-0', mode['Meta-0'], {})])]
call('ApplyMonitorsConfig', GLib.Variant('(uua(iiduba(ssa{sv}))a{sv})', (serial, 1, layout, {})))
"#;

fn deadline() -> Deadline {
    Deadline::after(20_000).expect("a deadline builds")
}

fn lease() -> InteractionLease {
    InteractionLease::guarded(deadline(), ()).expect("a lease")
}

struct Probe {
    display: DisplayInfo,
    log: PathBuf,
}

impl Probe {
    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    fn clear(&self) {
        std::fs::write(&self.log, "").expect("the probe log clears");
    }

    fn global(&self, x: f64, y: f64) -> Point {
        Point {
            x: self.display.bounds.x + x,
            y: self.display.bounds.y + y,
        }
    }

    fn wait_for(&self, what: &str, found: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let events = self.events();
            if found(&events) {
                return events;
            }
            assert!(
                Instant::now() < until,
                "{what} never reached the probe on {}: {events:?}",
                self.display.id
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn kind<'a>(events: &'a [Value], name: &str) -> Vec<&'a Value> {
    events.iter().filter(|event| event["ev"] == name).collect()
}

fn near(event: &Value, x: f64, y: f64) -> bool {
    (event["x"].as_f64().unwrap_or(f64::NAN) - x).abs() <= 1.0
        && (event["y"].as_f64().unwrap_or(f64::NAN) - y).abs() <= 1.0
}

fn start_pipewire(launched: &mut Launched) {
    let runtime = std::env::var("XDG_RUNTIME_DIR").expect("a private runtime dir");
    launched.spawn("pipewire", &[]);
    let socket = Path::new(&runtime).join("pipewire-0");
    let until = Instant::now() + Duration::from_secs(5);
    while std::os::unix::net::UnixStream::connect(&socket).is_err() {
        assert!(Instant::now() < until, "private PipeWire did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn open_probes(launched: &mut Launched) -> Vec<Probe> {
    let layout = Command::new("python3")
        .args(["-c", LAYOUT])
        .output()
        .unwrap();
    assert!(layout.status.success(), "{layout:?}");
    std::thread::sleep(Duration::from_secs(1));
    let displays = crate::system::list_wayland_displays(deadline()).unwrap();
    assert_eq!(displays.len(), 2, "{displays:?}");
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/e2e-linux/apps/pointer_probe_gtk4.py")
        .display()
        .to_string();
    let runtime = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap());
    let probes: Vec<Probe> = displays
        .into_iter()
        .map(|display| {
            let log = runtime.join(format!("pointer-probe-{}.jsonl", display.id));
            let path = log.display().to_string();
            launched.spawn("python3", &[&script, &path, &display.id]);
            Probe { display, log }
        })
        .collect();
    for probe in &probes {
        probe.wait_for("fullscreen", |events| {
            kind(events, "state").iter().any(|state| {
                state["fullscreen"] == true && state["on"] == probe.display.id.as_str()
            })
        });
    }
    std::thread::sleep(Duration::from_millis(500));
    probes
}

fn click(at: Point, button: MouseButton, count: u32, modifiers: Vec<Modifier>) -> MouseEvent {
    MouseEvent {
        point: at,
        button,
        kind: MouseEventKind::Click { count },
        modifiers,
    }
}

fn send(event: MouseEvent) -> agent_desktop_core::OperationReport {
    crate::input::deliver::mouse_event(&event, &lease()).expect("the compositor takes the event")
}

fn legs_on(probe: &Probe, other: &Probe) {
    for round in 0..5 {
        probe.clear();
        other.clear();
        let local = (120.0 + f64::from(round) * 10.0, 77.0);
        let report = send(click(
            probe.global(local.0, local.1),
            MouseButton::Left,
            1,
            vec![],
        ));
        let reported = serde_json::to_value(&report).unwrap().to_string();
        assert!(reported.contains("delivered_unverified"), "{reported}");
        let events = probe.wait_for("a left click", |events| !kind(events, "press").is_empty());
        let presses = kind(&events, "press");
        assert_eq!(presses.len(), 1, "{events:?}");
        assert!(near(presses[0], local.0, local.1), "{events:?}");
        assert_eq!(presses[0]["button"], 1);
        assert!(kind(&other.events(), "press").is_empty());
    }
    probe.clear();
    send(click(
        probe.global(300., 200.),
        MouseButton::Left,
        2,
        vec![],
    ));
    probe.wait_for("a double click", |events| {
        kind(events, "press").iter().any(|press| press["n"] == 2)
    });
    probe.clear();
    send(click(
        probe.global(310., 210.),
        MouseButton::Right,
        1,
        vec![Modifier::Ctrl],
    ));
    probe.wait_for("a ctrl right click", |events| {
        kind(events, "press")
            .iter()
            .any(|press| press["button"] == 3 && press["ctrl"] == true && near(press, 310., 210.))
    });
    probe.clear();
    send(MouseEvent {
        point: probe.global(222., 111.),
        button: MouseButton::Left,
        kind: MouseEventKind::Move,
        modifiers: vec![],
    });
    probe.wait_for("a move", |events| {
        kind(events, "motion")
            .iter()
            .any(|motion| near(motion, 222., 111.))
    });
    probe.clear();
    send(MouseEvent {
        point: probe.global(250., 250.),
        button: MouseButton::Left,
        kind: MouseEventKind::Wheel {
            delta_x: 1.,
            delta_y: 2.,
        },
        modifiers: vec![],
    });
    let events = probe.wait_for("a wheel turn", |events| kind(events, "scroll").len() >= 3);
    let scrolls = kind(&events, "scroll");
    let up: f64 = scrolls.iter().filter_map(|s| s["dy"].as_f64()).sum();
    let right: f64 = scrolls.iter().filter_map(|s| s["dx"].as_f64()).sum();
    assert!(
        up < 0.0 && right > 0.0,
        "positive dy scrolls up, positive dx right: {events:?}"
    );
    probe.clear();
    let params = DragParams {
        from: probe.global(100., 100.),
        to: probe.global(300., 200.),
        duration_ms: Some(200),
        drop_delay_ms: Some(50),
    };
    crate::input::deliver::drag(&params, &lease()).expect("the compositor takes the drag");
    let events = probe.wait_for("a drag", |events| !kind(events, "drag-end").is_empty());
    let begin = kind(&events, "drag-begin");
    let end = kind(&events, "drag-end");
    assert!(near(begin[0], 100., 100.), "{events:?}");
    assert!(
        (end[0]["dx"].as_f64().unwrap() - 200.).abs() <= 2.
            && (end[0]["dy"].as_f64().unwrap() - 100.).abs() <= 2.,
        "{events:?}"
    );
    println!(
        "{}: left x5, double, ctrl+right, move, wheel, drag landed at the requested logical points (scale {})",
        probe.display.id, probe.display.scale
    );
}

fn measure_clicks(probe: &Probe) {
    let mut samples = Vec::new();
    for index in 0..8 {
        probe.clear();
        let local = 400. + f64::from(index) * 5.;
        let started = Instant::now();
        send(click(
            probe.global(local, 300.),
            MouseButton::Left,
            1,
            vec![],
        ));
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
        probe.wait_for("a timed click", |events| !kind(events, "press").is_empty());
    }
    let mut kept = samples.split_off(1);
    kept.sort_by(f64::total_cmp);
    println!(
        "adapter --xy click on {}: min {:.1} ms, median {:.1} ms, max {:.1} ms (7 runs, warm-up dropped)",
        probe.display.id, kept[0], kept[3], kept[6]
    );
}

#[test]
#[ignore = "starts a two-monitor headless gnome-shell; run inside scripts/private-session.sh"]
fn coordinate_pointer_input_lands_where_the_probe_windows_say() {
    assert!(std::env::var_os("AGENT_DESKTOP_PRIVATE_SESSION").is_some());
    if std::env::var_os("AD_POINTER_WAYLAND_CHILD").is_none() {
        let output = Command::new("bash")
            .args(["scripts/private-wayland-shell.sh", "--"])
            .arg(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", FILTER, "--nocapture"])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .env("AD_POINTER_WAYLAND_CHILD", "1")
            .env("AGENT_DESKTOP_WAYLAND_MONITORS", MONITORS)
            .output()
            .unwrap();
        println!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(output.status.success(), "private Wayland child: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
            "{FILTER} must execute exactly one test"
        );
        return;
    }
    let mut launched = Launched::new();
    start_pipewire(&mut launched);
    let adapter = crate::LinuxAdapter::new();
    assert!(adapter.coordinate_pointer_refusal().is_none());
    let refusal = adapter
        .physical_pointer_refusal()
        .expect("an element-aimed pointer stays refused on Wayland");
    assert_eq!(refusal.code, ErrorCode::ActionNotSupported);
    let probes = open_probes(&mut launched);
    for (index, probe) in probes.iter().enumerate() {
        legs_on(probe, &probes[1 - index]);
    }
    let right_edge = probes
        .iter()
        .map(|probe| probe.display.bounds.x + probe.display.bounds.width)
        .fold(0.0, f64::max);
    let error = crate::input::deliver::mouse_event(
        &click(
            Point {
                x: right_edge + 10.,
                y: 10.,
            },
            MouseButton::Left,
            1,
            vec![],
        ),
        &lease(),
    )
    .expect_err("a point beyond every display is refused");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    for probe in &probes {
        measure_clicks(probe);
    }
}
