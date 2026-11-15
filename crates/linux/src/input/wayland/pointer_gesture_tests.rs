use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, DisplayInfo, DragParams, ErrorCode, Modifier,
    MouseButton, MouseEvent, MouseEventKind, Point, Rect,
};

use super::{PointerAct, WaylandGesture, run_gesture, translate};
use crate::input::keyboard::key_names::{KEYSYM_CONTROL_L, KEYSYM_SHIFT_L};
use crate::input::mouse::drag_plan::plan_drag;
use crate::input::wayland::key_session::KeySession;
use crate::input::wayland::pointer_send::{PointerSession, run_pointer};
use crate::input::wayland::stream_point::StreamPoint;

#[derive(Default)]
struct Recorder {
    log: Vec<String>,
    fail: Option<&'static str>,
}

impl Recorder {
    fn step(&mut self, event: String) -> Result<(), AdapterError> {
        let failed = self.fail.is_some_and(|fail| event.starts_with(fail));
        self.log.push(event);
        if failed {
            return Err(AdapterError::new(ErrorCode::ActionFailed, "failed"));
        }
        Ok(())
    }
}

impl KeySession for Recorder {
    fn create(&mut self, _: Deadline) -> Result<(), AdapterError> {
        self.step("create".into())
    }
    fn start(&mut self, _: Deadline) -> Result<(), AdapterError> {
        self.step("start".into())
    }
    fn keysym(&mut self, keysym: u32, down: bool, _: Deadline) -> Result<(), AdapterError> {
        self.step(format!("key:{keysym:x}:{down}"))
    }
    fn stop(&mut self, _: Deadline) -> Result<(), AdapterError> {
        self.step("stop".into())
    }
}

impl PointerSession for Recorder {
    fn button(&mut self, button: u32, down: bool, _: Deadline) -> Result<(), AdapterError> {
        self.step(format!("button:{button}:{down}"))
    }
    fn pause(&mut self, _: Duration) {}
    fn attach_stream(&mut self, connector: &str, _: Deadline) -> Result<(), AdapterError> {
        self.step(format!("attach:{connector}"))
    }
    fn motion_absolute(&mut self, at: &StreamPoint, _: Deadline) -> Result<(), AdapterError> {
        self.step(format!("motion:{}:{}:{}", at.stream, at.x, at.y))
    }
    fn axis_discrete(&mut self, axis: u32, steps: i32, _: Deadline) -> Result<(), AdapterError> {
        self.step(format!("axis:{axis}:{steps}"))
    }
}

fn deadline() -> Deadline {
    Deadline::after(10_000).expect("deadline")
}

fn display(id: &str, bounds: (f64, f64, f64, f64), scale: f64) -> DisplayInfo {
    DisplayInfo {
        id: id.into(),
        bounds: Rect {
            x: bounds.0,
            y: bounds.1,
            width: bounds.2,
            height: bounds.3,
        },
        is_primary: false,
        scale,
    }
}

fn two_monitors() -> Vec<DisplayInfo> {
    vec![
        display("Meta-0", (1600., 0., 1280., 800.), 1.),
        display("Meta-1+Meta-2", (0., 0., 800., 600.), 2.),
    ]
}

fn event(kind: MouseEventKind, x: f64, y: f64) -> MouseEvent {
    MouseEvent {
        point: Point { x, y },
        button: MouseButton::Left,
        kind,
        modifiers: Vec::new(),
    }
}

fn sent(log: &[String]) -> Vec<&str> {
    log.iter()
        .map(String::as_str)
        .filter(|event| {
            !matches!(*event, "create" | "start" | "stop") && !event.starts_with("attach")
        })
        .collect()
}

#[test]
fn a_point_is_sent_to_the_stream_of_its_monitor_in_stream_units() {
    let displays = two_monitors();
    let mut session = Recorder::default();
    run_pointer(
        &mut session,
        &event(MouseEventKind::Click { count: 1 }, 100., 150.),
        &displays,
        &mut || Ok(()),
        deadline(),
    )
    .expect("a click on the scaled monitor is sent");
    assert_eq!(session.log[1], "attach:Meta-1");
    assert_eq!(
        sent(&session.log),
        [
            "motion:0:200:300",
            "motion:0:200:300",
            "button:272:true",
            "button:272:false"
        ]
    );
    let mut session = Recorder::default();
    run_pointer(
        &mut session,
        &event(MouseEventKind::Move, 1700., 50.),
        &displays,
        &mut || Ok(()),
        deadline(),
    )
    .expect("a move on the unscaled monitor is sent");
    assert_eq!(session.log[1], "attach:Meta-0");
    assert_eq!(sent(&session.log), ["motion:0:100:50", "motion:0:100:50"]);
}

#[test]
fn a_point_between_monitors_is_refused_before_any_session_exists() {
    let mut session = Recorder::default();
    let error = run_pointer(
        &mut session,
        &event(MouseEventKind::Click { count: 1 }, 1000., 100.),
        &two_monitors(),
        &mut || Ok(()),
        deadline(),
    )
    .expect_err("no display holds the point");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(session.log.is_empty());
}

#[test]
fn a_double_click_presses_twice_after_one_lead_motion() {
    let mut session = Recorder::default();
    run_pointer(
        &mut session,
        &event(MouseEventKind::Click { count: 2 }, 1650., 10.),
        &two_monitors(),
        &mut || Ok(()),
        deadline(),
    )
    .expect("a double click is sent");
    let presses = sent(&session.log)
        .into_iter()
        .filter(|event| *event == "button:272:true")
        .count();
    assert_eq!(presses, 2);
}

#[test]
fn wheel_detents_follow_the_x11_direction_convention() {
    let mut session = Recorder::default();
    run_pointer(
        &mut session,
        &event(
            MouseEventKind::Wheel {
                delta_x: -1.,
                delta_y: 2.,
            },
            1650.,
            10.,
        ),
        &two_monitors(),
        &mut || Ok(()),
        deadline(),
    )
    .expect("a wheel turn is sent");
    assert_eq!(
        sent(&session.log),
        [
            "motion:0:50:10",
            "motion:0:50:10",
            "axis:0:-2",
            "motion:0:50:10",
            "axis:1:-1"
        ]
    );
}

#[test]
fn modifiers_wrap_the_gesture_after_a_lead_key() {
    let mut session = Recorder::default();
    let mut click = event(MouseEventKind::Click { count: 1 }, 1650., 10.);
    click.modifiers = vec![Modifier::Ctrl];
    click.button = MouseButton::Right;
    run_pointer(
        &mut session,
        &click,
        &two_monitors(),
        &mut || Ok(()),
        deadline(),
    )
    .expect("a modified click is sent");
    let shift = format!("key:{KEYSYM_SHIFT_L:x}");
    let ctrl = format!("key:{KEYSYM_CONTROL_L:x}");
    assert_eq!(
        sent(&session.log),
        [
            "motion:0:50:10".to_owned(),
            format!("{shift}:true"),
            format!("{shift}:false"),
            format!("{ctrl}:true"),
            "motion:0:50:10".to_owned(),
            "button:273:true".to_owned(),
            "button:273:false".to_owned(),
            format!("{ctrl}:false"),
        ]
    );
}

fn drag_gesture(from: (f64, f64), to: (f64, f64)) -> Result<WaylandGesture, AdapterError> {
    let plan = plan_drag(
        &DragParams {
            from: Point {
                x: from.0,
                y: from.1,
            },
            to: Point { x: to.0, y: to.1 },
            duration_ms: Some(100),
            drop_delay_ms: Some(0),
        },
        deadline(),
    )?;
    translate(&plan.steps, Vec::new(), &two_monitors())
}

#[test]
fn a_drag_across_monitors_links_both_streams_in_first_use_order() {
    let displays = [
        display("Meta-0", (0., 0., 1280., 800.), 1.),
        display("Meta-1", (1280., 0., 1280., 800.), 1.),
    ];
    let plan = plan_drag(
        &DragParams {
            from: Point { x: 1200., y: 400. },
            to: Point { x: 1400., y: 400. },
            duration_ms: Some(100),
            drop_delay_ms: Some(0),
        },
        deadline(),
    )
    .unwrap();
    let gesture = translate(&plan.steps, Vec::new(), &displays).unwrap();
    assert_eq!(gesture.connectors, ["Meta-0", "Meta-1"]);
    let last = gesture
        .acts
        .iter()
        .rev()
        .find_map(|act| match act {
            PointerAct::Motion(at) => Some(at.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        last,
        StreamPoint {
            stream: 1,
            x: 120.,
            y: 400.
        }
    );
    let error = drag_gesture((100., 100.), (1700., 100.))
        .expect_err("the path crosses the gap between the monitors");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_failure_mid_drag_releases_the_held_button() {
    let gesture = drag_gesture((1650., 10.), (1750., 10.)).unwrap();
    let mut session = Recorder {
        fail: Some("motion:0:64"),
        ..Recorder::default()
    };
    let error = run_gesture(&mut session, &gesture, &mut || Ok(()), deadline())
        .expect_err("the compositor failed a motion while the button was down");
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    let log = sent(&session.log);
    let pressed = log.iter().position(|event| *event == "button:272:true");
    let released = log.iter().rposition(|event| *event == "button:272:false");
    assert!(pressed.is_some() && released > pressed, "{log:?}");
    assert_eq!(session.log.last().map(String::as_str), Some("stop"));
}

#[test]
fn a_long_wheel_turn_sends_one_call_per_burst_of_ten() {
    let mut session = Recorder::default();
    run_pointer(
        &mut session,
        &event(
            MouseEventKind::Wheel {
                delta_x: 0.,
                delta_y: -25.,
            },
            1650.,
            10.,
        ),
        &two_monitors(),
        &mut || Ok(()),
        deadline(),
    )
    .expect("a long wheel turn is sent");
    let axes: Vec<&str> = sent(&session.log)
        .into_iter()
        .filter(|event| event.starts_with("axis"))
        .collect();
    assert_eq!(axes, ["axis:0:10", "axis:0:10", "axis:0:5"]);
}
