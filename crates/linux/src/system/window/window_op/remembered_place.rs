use agent_desktop_core::Rect;
use serde_json::{Value, json};

use super::window_op_verdict::POSITION_TOLERANCE;
use crate::system::x11::control::window_facts::WindowFacts;
use crate::tree::Extents;

/// The client rectangle a window had before the agent maximized it, tied to
/// the window state the agent last left it in. The place is given back only
/// while the live window still shows that state; any other state means the
/// user, the application or another tool changed the window since, and the
/// record is stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RememberedPlace {
    /// The agent maximized the window and observed it maximized at
    /// `maximized`.
    Maximized { before: Extents, maximized: Extents },
    /// The agent un-maximized the window and left it at `left` without
    /// giving `before` back; a retry may still give it back.
    Unmaximized { before: Extents, left: Extents },
}

impl RememberedPlace {
    /// The place to give back when `live` still shows the state the record
    /// was taken from, or `None` when the record is stale. A maximized record
    /// holds while the window is fully maximized on the display it was
    /// maximized on; an un-maximized record holds while the window is not
    /// maximized and still sits where the agent left it.
    pub(super) fn place_for(&self, live: &WindowFacts, displays: &[Rect]) -> Option<Extents> {
        let current = match self {
            Self::Maximized { before, maximized } => (live.maximized_vertical
                && live.maximized_horizontal
                && display_of(maximized, displays) == display_of(&live.client, displays))
            .then_some(*before),
            Self::Unmaximized { before, left } => {
                (!live.maximized_vertical && !live.maximized_horizontal && near(left, &live.client))
                    .then_some(*before)
            }
        };
        current.filter(|_| !live.hidden)
    }

    pub(super) fn to_json(self) -> Value {
        let (phase, before, at) = match self {
            Self::Maximized { before, maximized } => ("maximized", before, maximized),
            Self::Unmaximized { before, left } => ("unmaximized", before, left),
        };
        json!({ "phase": phase, "before": rectangle_json(before), "at": rectangle_json(at) })
    }

    pub(super) fn from_json(value: &Value) -> Option<Self> {
        let before = rectangle_of(&value["before"])?;
        let at = rectangle_of(&value["at"])?;
        match value["phase"].as_str()? {
            "maximized" => Some(Self::Maximized {
                before,
                maximized: at,
            }),
            "unmaximized" => Some(Self::Unmaximized { before, left: at }),
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

fn display_of(rectangle: &Extents, displays: &[Rect]) -> Option<usize> {
    let x = f64::from(rectangle.x) + f64::from(rectangle.width) / 2.0;
    let y = f64::from(rectangle.y) + f64::from(rectangle.height) / 2.0;
    displays.iter().position(|display| {
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
