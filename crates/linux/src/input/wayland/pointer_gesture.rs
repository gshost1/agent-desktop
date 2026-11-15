//! Runs a planned pointer gesture through a compositor RemoteDesktop session.
//! The plan is the same one X11 sends; its global points are mapped onto the
//! ScreenCast stream of the monitor under each point, and its buttons onto
//! evdev codes and wheel detents.

use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, DisplayInfo, ErrorCode, Point,
};
use serde_json::json;

use super::pointer_send::{PointerSession, delivery_error, execute_created_session};
use super::stream_point::StreamPoint;
use crate::input::delivery::{Delivery, Gate};
use crate::input::keyboard::key_names::KEYSYM_SHIFT_L;
use crate::input::physical::shell_aim::display_under;
use crate::input::step::Step;

/// A fresh session's first motion only creates the virtual pointer, and a
/// press before a second motion reaches no surface (L6 probe, 4 of 4 runs).
const LEAD_SETTLE: Duration = Duration::from_millis(100);
const MOTION_SETTLE: Duration = Duration::from_millis(50);
const KEY_LEAD_SETTLE: Duration = Duration::from_millis(100);
const RECOVERY_MS: u64 = 1_000;
const BTN_LEFT: u32 = 272;
const BTN_RIGHT: u32 = 273;
const BTN_MIDDLE: u32 = 274;
const AXIS_VERTICAL: u32 = 0;
const AXIS_HORIZONTAL: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PointerAct {
    Motion(StreamPoint),
    Button { code: u32, down: bool },
    Axis { axis: u32, steps: i32 },
    Pause(Duration),
}

/// A gesture ready for one session: the monitors it touches in link order,
/// what it sends, and the modifier keysyms held around it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WaylandGesture {
    pub(crate) connectors: Vec<String>,
    pub(crate) acts: Vec<PointerAct>,
    pub(crate) modifiers: Vec<u32>,
}

fn outside(x: i32, y: i32) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The point ({x}, {y}) is outside every display the compositor lists"),
    )
    .with_suggestion("Read list-displays and pass a point inside one display's bounds")
    .with_details(json!({ "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn stream_point(
    x: i32,
    y: i32,
    displays: &[DisplayInfo],
    connectors: &mut Vec<String>,
) -> Result<StreamPoint, AdapterError> {
    let point = Point {
        x: f64::from(x),
        y: f64::from(y),
    };
    let display = display_under(&point, displays).ok_or_else(|| outside(x, y))?;
    let connector = display.id.split('+').next().unwrap_or_default().to_owned();
    let stream = match connectors.iter().position(|known| *known == connector) {
        Some(index) => index,
        None => {
            connectors.push(connector);
            connectors.len() - 1
        }
    };
    let scale = if display.scale.is_finite() && display.scale > 0.0 {
        display.scale
    } else {
        1.0
    };
    Ok(StreamPoint {
        stream,
        x: (point.x - display.bounds.x) * scale,
        y: (point.y - display.bounds.y) * scale,
    })
}

fn button_act(code: u8, down: bool) -> Result<Option<PointerAct>, AdapterError> {
    let wheel = |axis, steps| down.then_some(PointerAct::Axis { axis, steps });
    Ok(match code {
        1 => Some(PointerAct::Button {
            code: BTN_LEFT,
            down,
        }),
        2 => Some(PointerAct::Button {
            code: BTN_MIDDLE,
            down,
        }),
        3 => Some(PointerAct::Button {
            code: BTN_RIGHT,
            down,
        }),
        4 => wheel(AXIS_VERTICAL, -1),
        5 => wheel(AXIS_VERTICAL, 1),
        6 => wheel(AXIS_HORIZONTAL, -1),
        7 => wheel(AXIS_HORIZONTAL, 1),
        _ => return Err(AdapterError::internal("The plan names no pointer button")),
    })
}

/// Maps a pointer plan onto compositor acts. Every point must lie on a listed
/// display, so nothing is sent for a gesture that would leave the screens.
pub(crate) fn translate(
    steps: &[Step],
    modifiers: Vec<u32>,
    displays: &[DisplayInfo],
) -> Result<WaylandGesture, AdapterError> {
    let mut connectors = Vec::new();
    let mut acts = Vec::new();
    for step in steps {
        match step {
            Step::MoveTo { x, y } => acts.push(PointerAct::Motion(stream_point(
                *x,
                *y,
                displays,
                &mut connectors,
            )?)),
            Step::Button { code, down } => acts.extend(button_act(*code, *down)?),
            Step::Pause(duration) => acts.push(PointerAct::Pause(*duration)),
            Step::Key { .. } | Step::CheckTextState | Step::Keymap(_) => {
                return Err(AdapterError::internal(
                    "A keyboard step reached the Wayland pointer route",
                ));
            }
        }
    }
    if !acts.iter().any(|act| matches!(act, PointerAct::Motion(_))) {
        return Err(AdapterError::internal("A pointer plan has no position"));
    }
    Ok(WaylandGesture {
        connectors,
        acts,
        modifiers,
    })
}

#[derive(Default)]
struct Progress {
    delivered: u32,
    buttons: Vec<u32>,
    keys: Vec<u32>,
    moved: bool,
}

fn sent<S: PointerSession>(
    progress: &mut Progress,
    session: &mut S,
    act: &PointerAct,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    match act {
        PointerAct::Motion(at) => session.motion_absolute(at, deadline),
        PointerAct::Button { code, down } => session.button(*code, *down, deadline),
        PointerAct::Axis { axis, steps } => session.axis_discrete(*axis, *steps, deadline),
        PointerAct::Pause(duration) => {
            session.pause(*duration);
            return Ok(());
        }
    }?;
    progress.delivered += 1;
    Ok(())
}

fn uncertain(error: AdapterError) -> AdapterError {
    delivery_error(error, DeliverySemantics::uncertain())
}

fn refused_or_uncertain(error: AdapterError) -> AdapterError {
    if error.disposition == DeliverySemantics::not_delivered() {
        delivery_error(error, DeliverySemantics::delivered_unverified())
    } else {
        uncertain(error)
    }
}

fn hold_keys<S: PointerSession>(
    session: &mut S,
    progress: &mut Progress,
    modifiers: &[u32],
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if modifiers.is_empty() {
        return Ok(());
    }
    for down in [true, false] {
        session
            .keysym(KEYSYM_SHIFT_L, down, deadline)
            .map_err(uncertain)?;
        progress.delivered += 1;
    }
    session.pause(KEY_LEAD_SETTLE);
    for keysym in modifiers {
        session.keysym(*keysym, true, deadline).map_err(uncertain)?;
        progress.keys.push(*keysym);
        progress.delivered += 1;
    }
    Ok(())
}

fn act_once<S: PointerSession>(
    session: &mut S,
    progress: &mut Progress,
    act: &PointerAct,
    gate: Gate<'_>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    deadline.ensure_remaining().map_err(uncertain)?;
    match act {
        PointerAct::Motion(_) => {
            sent(progress, session, act, deadline).map_err(uncertain)?;
            if !progress.moved {
                progress.moved = true;
                session.pause(MOTION_SETTLE);
            }
        }
        PointerAct::Button { code, down: true } => {
            gate().map_err(|error| {
                delivery_error(error, DeliverySemantics::delivered_unverified())
            })?;
            sent(progress, session, act, deadline).map_err(refused_or_uncertain)?;
            progress.buttons.push(*code);
        }
        PointerAct::Button { code, down: false } => {
            sent(progress, session, act, deadline).map_err(uncertain)?;
            progress.buttons.retain(|held| held != code);
        }
        PointerAct::Axis { .. } => {
            gate().map_err(|error| {
                delivery_error(error, DeliverySemantics::delivered_unverified())
            })?;
            sent(progress, session, act, deadline).map_err(refused_or_uncertain)?;
        }
        PointerAct::Pause(_) => sent(progress, session, act, deadline)?,
    }
    Ok(())
}

fn release<S: PointerSession>(session: &mut S, progress: &mut Progress) {
    let Ok(deadline) = Deadline::detached_after(RECOVERY_MS) else {
        return;
    };
    for code in progress.buttons.drain(..).rev() {
        let _ = session.button(code, false, deadline);
    }
    for keysym in progress.keys.drain(..).rev() {
        let _ = session.keysym(keysym, false, deadline);
    }
}

fn drive<S: PointerSession>(
    session: &mut S,
    gesture: &WaylandGesture,
    gate: Gate<'_>,
    deadline: Deadline,
) -> Result<u32, AdapterError> {
    let unsent = |mut error: AdapterError| {
        error.details.get_or_insert_with(|| json!({}))["physical_delivery_started"] = false.into();
        error.with_disposition(DeliverySemantics::not_delivered())
    };
    for connector in &gesture.connectors {
        session.attach_stream(connector, deadline).map_err(unsent)?;
    }
    session.start(deadline).map_err(unsent)?;
    gate()?;
    let mut progress = Progress::default();
    let outcome = lead_then_act(session, &mut progress, gesture, gate, deadline);
    if outcome.is_err() {
        release(session, &mut progress);
    }
    outcome.map(|()| progress.delivered)
}

fn lead_then_act<S: PointerSession>(
    session: &mut S,
    progress: &mut Progress,
    gesture: &WaylandGesture,
    gate: Gate<'_>,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if let Some(lead) = gesture
        .acts
        .iter()
        .find(|act| matches!(act, PointerAct::Motion(_)))
    {
        sent(progress, session, lead, deadline).map_err(uncertain)?;
        session.pause(LEAD_SETTLE);
    }
    hold_keys(session, progress, &gesture.modifiers, deadline)?;
    for act in &gesture.acts {
        act_once(session, progress, act, gate, deadline)?;
    }
    for keysym in gesture.modifiers.iter().rev() {
        session
            .keysym(*keysym, false, deadline)
            .map_err(uncertain)?;
        progress.keys.retain(|held| held != keysym);
        progress.delivered += 1;
    }
    Ok(())
}

/// Sends `gesture` through a session that was opened and not yet created.
/// The session is stopped on every path once it was created.
pub(crate) fn run_gesture<S: PointerSession>(
    session: &mut S,
    gesture: &WaylandGesture,
    gate: Gate<'_>,
    deadline: Deadline,
) -> Result<Delivery, AdapterError> {
    session.create(deadline)?;
    let mut delivered = 0;
    execute_created_session(session, deadline, |session| {
        delivered = drive(session, gesture, gate, deadline)?;
        Ok(())
    })?;
    Ok(Delivery {
        delivered_events: delivered,
    })
}

#[cfg(test)]
#[path = "pointer_gesture_tests.rs"]
mod tests;
