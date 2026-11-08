//! Confirm and cancel keys aimed at a sheet or dialog. A semantic `AXConfirm`
//! or `AXCancel` can succeed on the focused control while the surface ignores
//! it: a text field ends editing, but the controller that acts on Return never
//! sees a key. Such a press is judged by what happened to the surface itself,
//! never by the action's own success, and never by window geometry: a sheet
//! that resizes while it stays open has not reacted to the key.

use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
};
use std::time::{Duration, Instant};

use crate::tree::AXElement;

const SETTLE_LIMIT: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const DIALOG_SUBROLES: [&str; 2] = ["AXDialog", "AXSystemDialog"];
const SURFACE_CLOSED: &str = "surface_closed";
const SURFACE_REPLACED: &str = "surface_replaced";
const TEXT_EDITED: &str = "text_edited";

/// The sheet or dialog that holds the keyboard when a confirm or cancel key is
/// pressed, and the element that lists it: the window for a sheet, the
/// application for a dialog.
pub(super) struct ModalWatch {
    app: AXElement,
    surface: AXElement,
    container: (AXElement, &'static str),
    kind: &'static str,
    action: &'static str,
    button: (&'static str, bool),
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

    fn read(
        app: &AXElement,
        action: &'static str,
        deadline: Deadline,
    ) -> Result<Option<Self>, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let Some((surface, kind, window)) = modal_surface(app, instant)? else {
            return Ok(None);
        };
        let (attribute, key) = button_attribute(action);
        let present = crate::tree::surface_read::element(&surface, attribute, instant)?.is_some();
        let container = if kind == "sheet" {
            (window, "AXChildren")
        } else {
            (app.clone(), "AXWindows")
        };
        Ok(Some(Self {
            app: app.clone(),
            surface,
            container,
            kind,
            action,
            button: (key, present),
        }))
    }

    /// Waits a bounded time for a signal that the surface reacted, and returns
    /// its name. `edited` adds the text of a multi-line field that held focus
    /// when a real key was sent, where Return inserts a line.
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
                "The {kind} stayed open: the key was delivered, but the {kind} neither closed nor gave way to another",
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
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        if self.surface_closed(instant)? {
            return Ok(Some(SURFACE_CLOSED));
        }
        if self.surface_replaced(instant)? {
            return Ok(Some(SURFACE_REPLACED));
        }
        Ok(edited
            .is_some_and(|value| value.changed(deadline))
            .then_some(TEXT_EDITED))
    }

    fn surface_closed(&self, deadline: Instant) -> Result<bool, AdapterError> {
        let role = crate::tree::properties::attributes::copy_string_attr_result(
            &self.surface,
            "AXRole",
            deadline,
        );
        crate::tree::surface_read::ensure_before_deadline(deadline)?;
        let (container, attribute) = &self.container;
        let listed = crate::tree::surface_read::elements(container, attribute, deadline)
            .ok()
            .map(|elements| {
                elements
                    .iter()
                    .any(|element| crate::tree::same_element(element, &self.surface))
            });
        Ok(surface_shows_closed(role, listed))
    }

    fn surface_replaced(&self, deadline: Instant) -> Result<bool, AdapterError> {
        use crate::tree::surface_read;

        if let Some((current, _, _)) = modal_surface(&self.app, deadline)?
            && !crate::tree::same_element(&current, &self.surface)
        {
            return Ok(true);
        }
        for child in surface_read::elements(&self.surface, "AXChildren", deadline)? {
            if surface_read::string(&child, "AXRole", deadline)?.as_deref() == Some("AXSheet") {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// A multi-line text field that held focus just before a real key was sent,
/// with its text. Single-line fields are never read: a confirm that only
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
/// `limit` passes. A signal that one re-read does not confirm, such as a sheet
/// that is listed again, is not an effect.
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

/// A surface is closed when its element is invalid or roleless, or when the
/// element that listed it no longer does. An unreadable listing proves nothing.
fn surface_shows_closed(role: Result<Option<String>, i32>, listed: Option<bool>) -> bool {
    let element_gone = match role {
        Ok(role) => role.is_none(),
        Err(error) => error == accessibility_sys::kAXErrorInvalidUIElement,
    };
    element_gone || listed == Some(false)
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
