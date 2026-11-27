use crate::system::window::window_surface_watch;
use crate::tree::{AXElement, surface_read};
use agent_desktop_core::{ActionResult, ActionStepOutcome, Deadline, DeliverySemantics};
use std::time::Duration;

const SURFACE_ANCESTOR_WALK: usize = 12;
/// The settle the confirm and cancel keys on a sheet already use: long enough
/// for a sheet's close animation to order its window out, and paid in full only
/// by a press whose sheet or dialog stays open.
const SETTLE_LIMIT: Duration = Duration::from_millis(750);
/// A press below one of these acts on that surface, not on the sheet or dialog
/// beyond it, so the walk ends without a surface to watch.
const WALK_STOPS: [&str; 6] = [
    "AXApplication",
    "AXPopover",
    "AXMenu",
    "AXMenuItem",
    "AXMenuBar",
    "AXMenuBarItem",
];
const DIALOG_SUBROLES: [&str; 3] = ["AXDialog", "AXSystemDialog", "AXAlert"];
const SHEET: &str = "sheet";
const ALERT: &str = "alert";
const DETAIL: &str = "surface_after_press";

/// The sheet or alert that encloses a pressed element, watched through its own
/// window-server window. That window belongs to this one surface, so a second
/// sheet with the same title stacked on the same window cannot answer for it.
pub(crate) struct PressedSurface {
    kind: &'static str,
    window: Option<i64>,
}

impl PressedSurface {
    /// Nothing when the element is not inside a sheet or alert, so every other
    /// press pays only the parent walk. An unreadable ancestry is treated as no
    /// surface and never fails the press. A surface window that is not on screen
    /// before the press, such as one on another Space, is not watched, because
    /// its absence afterwards would prove nothing.
    pub(crate) fn capture(element: &AXElement, deadline: Deadline) -> Option<Self> {
        let end = crate::tree::locator_deadline::from_operation(deadline).ok()?;
        let (surface, kind) = enclosing_surface(
            element,
            |current| surface_read::string(current, "AXRole", end).ok().flatten(),
            |current| {
                surface_read::string(current, "AXSubrole", end)
                    .ok()
                    .flatten()
            },
            |current| {
                surface_read::element(current, "AXParent", end)
                    .ok()
                    .flatten()
            },
        )?;
        use crate::system::window::window_bridge::window_id;
        let own = window_id(&surface, end).ok().flatten();
        let host = surface_read::element(&surface, "AXParent", end)
            .ok()
            .flatten()
            .and_then(|host| window_id(&host, end).ok().flatten());
        let window = window_surface_watch::distinct_window(own, host)
            .filter(|window| window_surface_watch::window_on_screen(*window).unwrap_or(false));
        Some(Self { kind, window })
    }

    /// Judges an unverified press by whether its surface left the screen. Any
    /// other result is returned untouched, so a verified or undelivered press
    /// never waits here.
    pub(crate) fn judge(&self, result: ActionResult, deadline: Deadline) -> ActionResult {
        if result.disposition() != DeliverySemantics::delivered_unverified() {
            return result;
        }
        let Some(window) = self.window else {
            return self.report(result, Effect::Unobservable);
        };
        let settled = window_surface_watch::settle_with(
            || window_surface_watch::closed_signal(window),
            SETTLE_LIMIT,
            deadline,
        );
        let effect = match settled {
            Ok(Some(signal)) => Effect::Closed(signal),
            Ok(None) => Effect::StillShowing,
            Err(error) => {
                tracing::debug!(code = ?error.code, "click: pressed surface unreadable");
                Effect::Unobservable
            }
        };
        self.report(result, effect)
    }

    fn report(&self, mut result: ActionResult, effect: Effect) -> ActionResult {
        let mut detail = serde_json::json!({ "surface": self.kind, "effect": effect.name() });
        if let Effect::Closed(signal) = effect {
            detail["signal"] = signal.into();
            if let Some(step) = result
                .steps
                .iter_mut()
                .rev()
                .find(|step| matches!(step.outcome, ActionStepOutcome::Succeeded))
            {
                step.verified = Some(true);
            }
            result = result.with_verified_delivery();
        }
        let details = result.details.get_or_insert_with(|| serde_json::json!({}));
        if let Some(details) = details.as_object_mut() {
            details.insert(DETAIL.into(), detail);
        }
        result
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    Closed(&'static str),
    StillShowing,
    Unobservable,
}

impl Effect {
    fn name(self) -> &'static str {
        match self {
            Self::Closed(_) => "closed",
            Self::StillShowing => "still_showing",
            Self::Unobservable => "unobservable",
        }
    }
}

/// The nearest sheet, or the window itself when it is a sheet or dialog. A
/// standard window ends the walk without a surface.
fn enclosing_surface<E: Clone>(
    start: &E,
    role: impl Fn(&E) -> Option<String>,
    subrole: impl Fn(&E) -> Option<String>,
    parent: impl Fn(&E) -> Option<E>,
) -> Option<(E, &'static str)> {
    let mut current = start.clone();
    for _ in 0..SURFACE_ANCESTOR_WALK {
        let current_role = role(&current)?;
        if current_role == "AXSheet" {
            return Some((current, SHEET));
        }
        if current_role == "AXWindow" {
            let kind = window_surface_kind(subrole(&current).as_deref())?;
            return Some((current, kind));
        }
        if WALK_STOPS.contains(&current_role.as_str()) {
            return None;
        }
        current = parent(&current)?;
    }
    None
}

fn window_surface_kind(subrole: Option<&str>) -> Option<&'static str> {
    match subrole {
        Some("AXSheet") => Some(SHEET),
        Some(subrole) if DIALOG_SUBROLES.contains(&subrole) => Some(ALERT),
        _ => None,
    }
}

#[cfg(test)]
#[path = "activation_surface_tests.rs"]
mod tests;
