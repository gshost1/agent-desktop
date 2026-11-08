//! Keyboard focus held by a view that another process hosts inside the target
//! application's window.
//!
//! An out-of-process panel, such as the Open and Save panel of a sandboxed
//! application, draws inside a sheet window that the window server attributes
//! to the application, while its controls and its keyboard event queue belong
//! to the process that serves the panel. The application's own accessibility
//! focus can report the panel's control under the application's pid, so a key
//! posted to that pid never reaches the panel. The rules here decide, from
//! process relationships, focus and window-server evidence only, which process
//! must receive the application's keyboard input.

use agent_desktop_core::Rect;

use super::keyboard_route::KeyboardRoute;

const FRAME_TOLERANCE: f64 = 1.0;

/// One process other than the application that may own its keyboard focus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) pid: i32,
    /// The application's focused element reports this pid.
    pub(crate) owns_host_focus: bool,
    /// The system-wide focused application is this process.
    pub(crate) focused_application: bool,
    /// macOS names the application as responsible for this process.
    pub(crate) responsible_to_host: bool,
    /// This process's own focused element has the application's focus frame;
    /// `None` when it was not read.
    pub(crate) focus_frame_matches: Option<bool>,
    pub(crate) owns_window: bool,
}

impl Candidate {
    /// Why this process is not proven to own the application's keyboard focus.
    pub(crate) fn rejection(&self) -> Option<&'static str> {
        if !self.focused_application && !self.owns_window {
            return Some("owner_has_no_window_server_presence");
        }
        let same_control = self.owns_host_focus
            || self.focus_frame_matches == Some(true)
            || (self.focused_application && self.responsible_to_host);
        if !same_control {
            return Some("owner_focus_not_linked");
        }
        None
    }

    /// A process with no tie to the application's focus is not worth
    /// reporting unless the focused element sits outside any app window.
    pub(crate) fn is_relevant(&self, host_focus_has_window: bool) -> bool {
        self.owns_host_focus
            || self.focused_application
            || self.focus_frame_matches == Some(true)
            || (self.responsible_to_host && !host_focus_has_window)
    }

    pub(crate) fn evidence(&self) -> serde_json::Value {
        serde_json::json!({
            "pid": self.pid,
            "owns_host_focus": self.owns_host_focus,
            "focused_application": self.focused_application,
            "responsible_to_host": self.responsible_to_host,
            "focus_frame_matches": self.focus_frame_matches,
            "owns_window": self.owns_window,
        })
    }
}

/// `focus_inside_host` is whether the application's focused element lies in a
/// visible window the application owns, `None` when its frame is unknown.
pub(crate) fn decide_route(
    focus_inside_host: Option<bool>,
    candidates: &[Candidate],
) -> KeyboardRoute {
    let Some(first) = candidates.first() else {
        return KeyboardRoute::Host;
    };
    let reason = match focus_inside_host {
        None => Some("focus_frame_unknown"),
        Some(false) => Some("focus_outside_host_windows"),
        Some(true) => None,
    };
    if let Some(reason) = reason {
        return KeyboardRoute::Unproven {
            pid: first.pid,
            reason,
        };
    }
    if let Some(proven) = candidates
        .iter()
        .find(|candidate| candidate.rejection().is_none())
    {
        return KeyboardRoute::HostedView { pid: proven.pid };
    }
    KeyboardRoute::Unproven {
        pid: first.pid,
        reason: first.rejection().unwrap_or("owner_focus_not_linked"),
    }
}

pub(crate) fn frame_inside(frame: Option<Rect>, windows: &[Rect]) -> Option<bool> {
    let frame = frame.filter(|frame| frame.width > 0.0 && frame.height > 0.0)?;
    Some(windows.iter().any(|window| contains(window, &frame)))
}

pub(crate) fn same_frame(a: &Rect, b: &Rect) -> bool {
    (a.x - b.x).abs() <= FRAME_TOLERANCE
        && (a.y - b.y).abs() <= FRAME_TOLERANCE
        && (a.width - b.width).abs() <= FRAME_TOLERANCE
        && (a.height - b.height).abs() <= FRAME_TOLERANCE
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
