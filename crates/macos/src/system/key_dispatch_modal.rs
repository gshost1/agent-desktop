//! Confirm and cancel keys aimed at a sheet or dialog. A semantic `AXConfirm`
//! or `AXCancel` can succeed on the focused control while the surface ignores
//! it, and accessibility can report a sheet as gone while it is still on
//! screen. A press is verified only by a postcondition the window server
//! shows: the surface's own window has left the screen. Window geometry and
//! accessibility listings never count.

use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
};
use std::time::{Duration, Instant};

use crate::tree::AXElement;

const SETTLE_LIMIT: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const DIALOG_SUBROLES: [&str; 2] = ["AXDialog", "AXSystemDialog"];
const SURFACE_WINDOW_CLOSED: &str = "surface_window_closed";
const TEXT_EDITED: &str = "text_edited";

/// The sheet or dialog that holds the keyboard when a confirm or cancel key is
/// pressed, with its own window-server window when that can be told apart
/// from the window it is attached to.
pub(super) struct ModalWatch {
    kind: &'static str,
    action: &'static str,
    button: (&'static str, bool),
    window: Option<i64>,
}

impl ModalWatch {
    /// Returns a watch when the focused window is a dialog or carries a sheet,
    /// and nothing for every other press, which then runs unchanged. A surface
    /// that cannot be read is treated as absent, so a slow read never blocks a
    /// key that worked before; only an exhausted deadline fails the press.
    pub(super) fn capture(
        app: &AXElement,
        action: &'static str,
        deadline: Deadline,
    ) -> Result<Option<Self>, AdapterError> {
        match Self::read(app, action, deadline) {
            Ok(watch) => Ok(watch),
            Err(error) => {
                deadline.ensure_remaining()?;
                tracing::debug!(code = ?error.code, "keyboard: modal surface unreadable");
                Ok(None)
            }
        }
    }

    pub(super) fn action(&self) -> &'static str {
        self.action
    }

    /// Whether a reaction can be observed at all. Without the surface's own
    /// window there is no postcondition to show, and a press reports an
    /// unverified delivery instead of trying a second route.
    pub(super) fn observable(&self) -> bool {
        self.window.is_some()
    }

    fn read(
        app: &AXElement,
        action: &'static str,
        deadline: Deadline,
    ) -> Result<Option<Self>, AdapterError> {
        use crate::system::window::window_bridge::window_id;

        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let Some((surface, kind, window)) = modal_surface(app, instant)? else {
            return Ok(None);
        };
        let (attribute, key) = button_attribute(action);
        let present = crate::tree::surface_read::element(&surface, attribute, instant)?.is_some();
        let host = if crate::tree::same_element(&surface, &window) {
            crate::tree::surface_read::element(&surface, "AXParent", instant)
                .ok()
                .flatten()
        } else {
            Some(window)
        };
        let own = window_id(&surface, instant).ok().flatten();
        let host = host.and_then(|host| window_id(&host, instant).ok().flatten());
        Ok(Some(Self {
            kind,
            action,
            button: (key, present),
            window: distinct_window(own, host),
        }))
    }

    /// Waits a bounded time for a postcondition and returns its name. `edited`
    /// adds the text of a multi-line field that held focus when a real key was
    /// sent, where Return inserts a line.
    pub(super) fn settle(
        &self,
        edited: Option<&FocusedValue>,
        deadline: Deadline,
    ) -> Result<Option<&'static str>, AdapterError> {
        settle_with(|| self.signal(edited, deadline), SETTLE_LIMIT, deadline)
    }

    pub(super) fn verified(&self, steps: Vec<ActionStep>, signal: &str) -> ActionResult {
        let observed_after = steps.last().map(|step| step.label().to_string());
        ActionResult::delivered_unverified("press_key")
            .with_steps(steps)
            .with_verified_delivery()
            .with_details(serde_json::json!({
                "modal_key": {
                    "surface": self.kind,
                    "effect": "observed",
                    "signal": signal,
                    "observed_after": observed_after,
                }
            }))
    }

    pub(super) fn unobservable(&self, steps: Vec<ActionStep>) -> ActionResult {
        ActionResult::delivered_unverified("press_key")
            .with_steps(steps)
            .with_details(serde_json::json!({
                "modal_key": { "surface": self.kind, "effect": "unobservable" }
            }))
    }

    pub(super) fn ignored(&self, steps: &[ActionStep]) -> AdapterError {
        let routes: Vec<&str> = steps.iter().map(ActionStep::label).collect();
        let (button, present) = self.button;
        let mut report = serde_json::json!({
            "surface": self.kind,
            "effect": "none_observed",
            "routes": routes,
        });
        report[button] = serde_json::json!(present);
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!(
                "The {kind} stayed open: the key was delivered, but the {kind}'s window is still on screen",
                kind = self.kind
            ),
        )
        .with_details(serde_json::json!({ "modal_key": report }))
        .with_disposition(DeliverySemantics::delivered_unverified())
        .with_suggestion(ignored_suggestion(self.kind, self.button))
    }

    fn signal(
        &self,
        edited: Option<&FocusedValue>,
        deadline: Deadline,
    ) -> Result<Option<&'static str>, AdapterError> {
        if let Some(window) = self.window
            && !window_on_screen(window)?
        {
            return Ok(Some(SURFACE_WINDOW_CLOSED));
        }
        Ok(edited
            .is_some_and(|value| value.changed(deadline))
            .then_some(TEXT_EDITED))
    }
}

/// A multi-line text field that held focus just before a real key was sent,
/// with its text. Single-line fields are never read: a key that only
/// completed or rewrote their text left the surface open.
pub(super) struct FocusedValue {
    element: AXElement,
    value: Option<String>,
}

impl FocusedValue {
    pub(super) fn read(app: &AXElement, deadline: Deadline) -> Option<Self> {
        let element = super::focused_element(app, deadline).ok().flatten()?;
        let role = crate::tree::properties::attributes::copy_string_attr_result(
            &element, "AXRole", deadline,
        );
        if role.ok().flatten().as_deref() != Some("AXTextArea") {
            return None;
        }
        let value = crate::tree::copy_value_typed(&element, deadline);
        Some(Self { element, value })
    }

    fn changed(&self, deadline: Deadline) -> bool {
        crate::tree::copy_value_typed(&self.element, deadline) != self.value
    }
}

/// Polls `read` until the same signal is seen on two consecutive reads, or
/// `limit` passes. A signal that one re-read does not confirm is not an
/// effect.
fn settle_with(
    mut read: impl FnMut() -> Result<Option<&'static str>, AdapterError>,
    limit: Duration,
    deadline: Deadline,
) -> Result<Option<&'static str>, AdapterError> {
    let soft = Instant::now() + deadline.capped(limit).remaining();
    let hard = soft + POLL_INTERVAL * 3;
    let mut previous = None;
    loop {
        let signal = read()?;
        if signal.is_some() && signal == previous {
            return Ok(signal);
        }
        previous = signal;
        let now = Instant::now();
        if now >= hard || (now >= soft && previous.is_none()) {
            return Ok(None);
        }
        let pause = deadline.remaining_slice(POLL_INTERVAL)?;
        std::thread::sleep(pause.min(POLL_INTERVAL));
    }
}

/// The surface's own window, only when it differs from the window it is
/// attached to; a bridge that answers with the host's window would make the
/// surface look open forever or closed with the host.
fn distinct_window(own: Option<i64>, host: Option<i64>) -> Option<i64> {
    own.filter(|own| host != Some(*own))
}

fn window_on_screen(window: i64) -> Result<bool, AdapterError> {
    use crate::system::window::cg_window::{
        WindowRecordScope, records_from_dictionaries, window_dictionaries,
    };

    Ok(
        records_from_dictionaries(window_dictionaries()?, WindowRecordScope::Window(window))?
            .iter()
            .any(|record| record.visible),
    )
}

type FoundSurface = (AXElement, &'static str, AXElement);

fn modal_surface(app: &AXElement, deadline: Instant) -> Result<Option<FoundSurface>, AdapterError> {
    use crate::tree::surface_read;

    let Some(window) = surface_read::element(app, "AXFocusedWindow", deadline)? else {
        return Ok(None);
    };
    let role = surface_read::string(&window, "AXRole", deadline)?;
    let subrole = surface_read::string(&window, "AXSubrole", deadline)?;
    if let Some(kind) = modal_kind(role.as_deref(), subrole.as_deref()) {
        return Ok(Some((window.clone(), kind, window)));
    }
    for child in surface_read::elements(&window, "AXChildren", deadline)? {
        if surface_read::string(&child, "AXRole", deadline)?.as_deref() == Some("AXSheet") {
            return Ok(Some((child, "sheet", window)));
        }
    }
    Ok(None)
}

fn modal_kind(role: Option<&str>, subrole: Option<&str>) -> Option<&'static str> {
    if role == Some("AXSheet") || subrole == Some("AXSheet") {
        return Some("sheet");
    }
    subrole
        .is_some_and(|subrole| DIALOG_SUBROLES.contains(&subrole))
        .then_some("dialog")
}

fn button_attribute(action: &str) -> (&'static str, &'static str) {
    if action == "AXCancel" {
        ("AXCancelButton", "cancel_button")
    } else {
        ("AXDefaultButton", "default_button")
    }
}

fn ignored_suggestion(kind: &str, (button, present): (&str, bool)) -> String {
    let surface = if kind == "sheet" { "sheet" } else { "alert" };
    let route = match (present, button) {
        (true, "cancel_button") => "click its cancel button by ref",
        (true, _) => "click its default button by ref",
        (false, _) => {
            "send the key once more with --headed so its window takes focus first, or click its Close or Cancel button by ref to dismiss it"
        }
    };
    format!(
        "Do not repeat the key as is: read the {kind} again with 'snapshot --app <app> --surface {surface} -i', then {route}"
    )
}

#[cfg(test)]
#[path = "key_dispatch_modal_tests.rs"]
mod tests;
