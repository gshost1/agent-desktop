//! Keyboard focus held by a view that another process hosts inside the target
//! application's window.
//!
//! An out-of-process panel, such as the Open and Save panel of a sandboxed
//! application, draws inside a sheet window that the window server attributes
//! to the application, while its controls and its keyboard event queue belong
//! to the process that serves the panel. A key event posted to the
//! application's pid never reaches such a panel. The rules here decide, from
//! observed focus ownership and window geometry only, when keyboard input for
//! the application must go to the serving process instead.

use agent_desktop_core::Rect;

const FRAME_TOLERANCE: f64 = 1.0;

/// Process ids observed for the keyboard focus of one application.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FocusPids {
    pub(crate) host: i32,
    pub(crate) host_focus: Option<i32>,
    pub(crate) system_focus: Option<i32>,
    pub(crate) system_focus_is_host_focus: bool,
    pub(crate) focused_application: Option<i32>,
}

/// Window-server evidence that a focused view sits inside the host's window
/// and that its owning process can receive keyboard events.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HostingEvidence<'a> {
    pub(crate) focus_frame: Option<Rect>,
    pub(crate) host_windows: &'a [Rect],
    pub(crate) candidate_has_window: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyboardRoute {
    Host,
    HostedView { pid: i32 },
    Unproven { pid: i32, reason: &'static str },
}

impl KeyboardRoute {
    pub(crate) fn hosted_pid(self) -> Option<i32> {
        match self {
            Self::HostedView { pid } => Some(pid),
            Self::Host | Self::Unproven { .. } => None,
        }
    }
}

/// The process other than the host that the host's own focus points into, if
/// any. The system-wide focus counts only when it is the host's focused
/// element, so a different application's focus never redirects input.
pub(crate) fn focus_owner_candidate(pids: &FocusPids) -> Option<i32> {
    let foreign = |pid: Option<i32>| pid.filter(|pid| *pid > 0 && *pid != pids.host);
    foreign(pids.host_focus).or_else(|| {
        if pids.system_focus_is_host_focus {
            foreign(pids.system_focus)
        } else {
            None
        }
    })
}

pub(crate) fn decide_route(pids: &FocusPids, evidence: &HostingEvidence<'_>) -> KeyboardRoute {
    let Some(pid) = focus_owner_candidate(pids) else {
        return KeyboardRoute::Host;
    };
    let Some(frame) = evidence.focus_frame.filter(has_area) else {
        return KeyboardRoute::Unproven {
            pid,
            reason: "focus_frame_unknown",
        };
    };
    if !evidence
        .host_windows
        .iter()
        .any(|window| contains(window, &frame))
    {
        return KeyboardRoute::Unproven {
            pid,
            reason: "focus_outside_host_windows",
        };
    }
    if pids.focused_application != Some(pid) && !evidence.candidate_has_window {
        return KeyboardRoute::Unproven {
            pid,
            reason: "owner_has_no_window_server_presence",
        };
    }
    KeyboardRoute::HostedView { pid }
}

pub(crate) fn same_frame(a: &Rect, b: &Rect) -> bool {
    (a.x - b.x).abs() <= FRAME_TOLERANCE
        && (a.y - b.y).abs() <= FRAME_TOLERANCE
        && (a.width - b.width).abs() <= FRAME_TOLERANCE
        && (a.height - b.height).abs() <= FRAME_TOLERANCE
}

pub(crate) fn route_details(host: i32, route: KeyboardRoute) -> Option<serde_json::Value> {
    match route {
        KeyboardRoute::Host => None,
        KeyboardRoute::HostedView { pid } => Some(serde_json::json!({
            "pid": pid,
            "host_pid": host,
            "route": "hosted_view",
        })),
        KeyboardRoute::Unproven { pid, reason } => Some(serde_json::json!({
            "pid": host,
            "host_pid": host,
            "route": "host",
            "focus_owner_pid": pid,
            "unproven": reason,
        })),
    }
}

fn has_area(frame: &Rect) -> bool {
    frame.width > 0.0 && frame.height > 0.0
}

fn contains(outer: &Rect, inner: &Rect) -> bool {
    inner.x >= outer.x - FRAME_TOLERANCE
        && inner.y >= outer.y - FRAME_TOLERANCE
        && inner.x + inner.width <= outer.x + outer.width + FRAME_TOLERANCE
        && inner.y + inner.height <= outer.y + outer.height + FRAME_TOLERANCE
}

#[cfg(test)]
#[path = "hosted_focus_tests.rs"]
mod tests;
