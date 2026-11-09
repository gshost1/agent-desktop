use agent_desktop_core::Rect;
use serde_json::{Value, json};

use super::window_op_verdict::POSITION_TOLERANCE;
use crate::system::x11::control::window_facts::WindowFacts;
use crate::tree::Extents;

/// The client rectangle a window had before the agent maximized it, tied to
/// the window state the agent last left it in. The place is given back only
/// while the live window still shows that state; a shown window in any other
/// state was changed by the user, the application or another tool since, and
/// the record is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RememberedPlace {
    /// The agent asked for a maximize and has not confirmed where the
    /// maximized window settled; recorded before the write, so it replaces
    /// any record of an earlier cycle.
    Pending { before: Extents },
    /// The agent maximized the window and confirmed it held `maximized`.
    Maximized { before: Extents, maximized: Extents },
    /// The agent un-maximized the window and left it at `left` without
    /// giving `before` back; a retry may still give it back.
    Unmaximized { before: Extents, left: Extents },
}

impl RememberedPlace {
    /// The place to give back when `live` still shows the state the record
    /// was taken from. A pending record holds while the window is fully
    /// maximized; a maximized record also needs the window on the display,
    /// named by its bounds, that held the confirmed maximized rectangle; an
    /// un-maximized record holds while the window is not maximized and still
    /// sits where the agent left it.
    pub(super) fn place_for(&self, live: &WindowFacts, displays: &[Rect]) -> Option<Extents> {
        let maximized = live.maximized_vertical && live.maximized_horizontal;
        let holds = match self {
            Self::Pending { .. } => maximized,
            Self::Maximized { maximized: at, .. } => {
                maximized && display_of(at, displays) == display_of(&live.client, displays)
            }
            Self::Unmaximized { left, .. } => {
                !live.maximized_vertical && !live.maximized_horizontal && near(left, &live.client)
            }
        };
        holds.then_some(self.before())
    }

    /// The place to record before a new maximize of a window shown at
    /// `current`: the first place still owed when a restore left the window
    /// here, otherwise `current` itself.
    pub(super) fn carried(record: Option<Self>, current: &Extents) -> Extents {
        match record {
            Some(Self::Unmaximized { before, left }) if near(&left, current) => before,
            _ => *current,
        }
    }

    pub(super) fn before(&self) -> Extents {
        match self {
            Self::Pending { before }
            | Self::Maximized { before, .. }
            | Self::Unmaximized { before, .. } => *before,
        }
    }

    pub(super) fn to_json(self) -> Value {
        let (phase, at) = match self {
            Self::Pending { .. } => ("pending", Value::Null),
            Self::Maximized { maximized, .. } => ("maximized", rectangle_json(maximized)),
            Self::Unmaximized { left, .. } => ("unmaximized", rectangle_json(left)),
        };
        json!({ "phase": phase, "before": rectangle_json(self.before()), "at": at })
    }

    pub(super) fn from_json(value: &Value) -> Option<Self> {
        let before = rectangle_of(&value["before"])?;
        match value["phase"].as_str()? {
            "pending" => Some(Self::Pending { before }),
            "maximized" => Some(Self::Maximized {
                before,
                maximized: rectangle_of(&value["at"])?,
            }),
            "unmaximized" => Some(Self::Unmaximized {
                before,
                left: rectangle_of(&value["at"])?,
            }),
            _ => None,
        }
    }
}

fn near(a: &Extents, b: &Extents) -> bool {
    (a.x - b.x).abs() <= POSITION_TOLERANCE
        && (a.y - b.y).abs() <= POSITION_TOLERANCE
        && (a.width - b.width).abs() <= POSITION_TOLERANCE
        && (a.height - b.height).abs() <= POSITION_TOLERANCE
}

fn display_of(rectangle: &Extents, displays: &[Rect]) -> Option<Rect> {
    let x = f64::from(rectangle.x) + f64::from(rectangle.width) / 2.0;
    let y = f64::from(rectangle.y) + f64::from(rectangle.height) / 2.0;
    displays.iter().copied().find(|display| {
        x >= display.x
            && y >= display.y
            && x < display.x + display.width
            && y < display.y + display.height
    })
}

fn rectangle_json(rectangle: Extents) -> Value {
    json!({
        "x": rectangle.x,
        "y": rectangle.y,
        "width": rectangle.width,
        "height": rectangle.height,
    })
}

fn rectangle_of(value: &Value) -> Option<Extents> {
    let field = |name: &str| i32::try_from(value[name].as_i64()?).ok();
    Some(Extents {
        x: field("x")?,
        y: field("y")?,
        width: field("width")?,
        height: field("height")?,
    })
}

#[cfg(test)]
#[path = "remembered_place_tests.rs"]
mod tests;
