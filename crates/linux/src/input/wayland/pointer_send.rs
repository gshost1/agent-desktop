use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, DisplayInfo, DragParams, MouseEvent, MouseEventKind,
};
use serde_json::json;

use super::key_session::KeySession;
use super::pointer_gesture::{WaylandGesture, run_gesture, translate};
use super::remote_desktop::RemoteDesktop;
use super::stream_point::StreamPoint;
use crate::input::delivery::{Delivery, Gate};
use crate::input::keyboard::key_names::modifier_keysym;
use crate::input::keyboard::keymap::Keymap;
use crate::input::mouse::drag_plan::plan_drag;
use crate::input::mouse::pointer_plan::{plan_click, plan_move};
use crate::input::mouse::wheel_plan::plan_wheel;
use crate::input::standalone::reject_standalone;

const RECOVERY_MS: u64 = 1_000;

pub(crate) trait PointerSession: KeySession {
    fn button(&mut self, button: u32, down: bool, deadline: Deadline) -> Result<(), AdapterError>;
    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
    fn attach_stream(&mut self, connector: &str, deadline: Deadline) -> Result<(), AdapterError>;
    fn motion_absolute(&mut self, at: &StreamPoint, deadline: Deadline)
    -> Result<(), AdapterError>;
    fn axis_discrete(
        &mut self,
        axis: u32,
        steps: i32,
        deadline: Deadline,
    ) -> Result<(), AdapterError>;
}

impl PointerSession for RemoteDesktop {
    fn button(&mut self, button: u32, down: bool, deadline: Deadline) -> Result<(), AdapterError> {
        self.pointer_button(button, down, deadline)
    }

    fn attach_stream(&mut self, connector: &str, deadline: Deadline) -> Result<(), AdapterError> {
        RemoteDesktop::attach_stream(self, connector, deadline)
    }

    fn motion_absolute(
        &mut self,
        at: &StreamPoint,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.pointer_motion_absolute(at, deadline)
    }

    fn axis_discrete(
        &mut self,
        axis: u32,
        steps: i32,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.pointer_axis_discrete(axis, steps, deadline)
    }
}

pub(crate) fn delivery_error(error: AdapterError, disposition: DeliverySemantics) -> AdapterError {
    let details = error.details.clone().unwrap_or_else(|| json!({}));
    let mut details = match details {
        serde_json::Value::Object(details) => details,
        other => serde_json::Map::from_iter([("cause".to_owned(), other)]),
    };
    details.insert("physical_delivery_started".to_owned(), json!(true));
    error
        .with_details(serde_json::Value::Object(details))
        .with_disposition(disposition)
        .with_suggestion("Inspect the target with 'snapshot' before retrying; some physical input may already have been delivered")
}

pub(crate) fn execute_created_session<S: PointerSession>(
    session: &mut S,
    deadline: Deadline,
    execute: impl FnOnce(&mut S) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    match (execute(session), session.stop(recovery_deadline(deadline))) {
        (Ok(()), Err(error)) => Err(delivery_error(
            error,
            DeliverySemantics::delivered_unverified(),
        )),
        (result, _) => result,
    }
}

/// Release and Stop get their own budget after a click exhausts its deadline.
fn recovery_deadline(spent: Deadline) -> Deadline {
    Deadline::detached_after(RECOVERY_MS).unwrap_or(spent)
}

fn event_gesture(
    event: &MouseEvent,
    displays: &[DisplayInfo],
) -> Result<WaylandGesture, AdapterError> {
    let map = Keymap::default();
    let plan = match &event.kind {
        MouseEventKind::Move => plan_move(event.point.clone()),
        MouseEventKind::Click { count } => {
            plan_click(&map, event.point.clone(), &event.button, *count, &[])
        }
        MouseEventKind::Wheel { delta_x, delta_y } => {
            plan_wheel(&map, event.point.clone(), *delta_x, *delta_y, &[]).map(|plans| {
                crate::input::plan::Plan {
                    steps: plans.into_iter().flat_map(|plan| plan.steps).collect(),
                    release_origin: None,
                }
            })
        }
        MouseEventKind::Down => Err(reject_standalone("mouse_event down")),
        MouseEventKind::Up => Err(reject_standalone("mouse_event up")),
    }?;
    let modifiers = event.modifiers.iter().map(modifier_keysym).collect();
    translate(&plan.steps, modifiers, displays)
}

/// Sends one pointer event through a session opened for it; `gate` is asked
/// before the first event and before every press.
pub(crate) fn send(
    event: &MouseEvent,
    displays: &[DisplayInfo],
    gate: Gate<'_>,
    deadline: Deadline,
) -> Result<Delivery, AdapterError> {
    let gesture = event_gesture(event, displays)?;
    let mut session = RemoteDesktop::open(deadline)?;
    run_gesture(&mut session, &gesture, gate, deadline)
}

pub(crate) fn run_pointer<S: PointerSession>(
    session: &mut S,
    event: &MouseEvent,
    displays: &[DisplayInfo],
    gate: Gate<'_>,
    deadline: Deadline,
) -> Result<Delivery, AdapterError> {
    let gesture = event_gesture(event, displays)?;
    run_gesture(session, &gesture, gate, deadline)
}

/// A pointer event at caller-given coordinates. Nothing can prove which
/// window is under the point, so the caller reports it unverified.
pub(crate) fn mouse_event(
    event: &MouseEvent,
    deadline: Deadline,
) -> Result<Delivery, AdapterError> {
    let displays = crate::system::list_wayland_displays(deadline)?;
    send(event, &displays, &mut || Ok(()), deadline)
}

/// A drag between two caller-given points, which may lie on different
/// monitors; every point it passes must be on a listed display.
pub(crate) fn drag(params: &DragParams, deadline: Deadline) -> Result<Delivery, AdapterError> {
    let plan = plan_drag(params, deadline)?;
    let displays = crate::system::list_wayland_displays(deadline)?;
    let gesture = translate(&plan.steps, Vec::new(), &displays)?;
    let mut session = RemoteDesktop::open(deadline)?;
    run_gesture(&mut session, &gesture, &mut || Ok(()), deadline)
}

#[cfg(test)]
#[path = "target_pointer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pointer_live_tests.rs"]
mod live_tests;
