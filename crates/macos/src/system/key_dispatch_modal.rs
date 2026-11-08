//! Confirm and cancel keys aimed at a sheet or dialog. A semantic `AXConfirm`
//! or `AXCancel` can succeed on the focused control while the surface ignores
//! it: a text field ends editing, but the controller that acts on Return never
//! sees a key. Such a press is judged by its effect on the surface, never by
//! the action's own success, and a real key is sent when the action changed
//! nothing.

use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
};
use std::time::{Duration, Instant};

use crate::system::window::cg_window::{WindowRecord, WindowRecordScope};
use crate::tree::AXElement;

const SETTLE_LIMIT: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const DIALOG_SUBROLES: [&str; 2] = ["AXDialog", "AXSystemDialog"];

/// The sheet or dialog that holds the keyboard when a confirm or cancel key is
/// pressed, with the application's window-server windows before delivery.
pub(super) struct ModalWatch {
    surface: AXElement,
    kind: &'static str,
    action: &'static str,
    pid: i32,
    button: (&'static str, bool),
    windows: Vec<WindowRecord>,
}

impl ModalWatch {
    /// Returns a watch when the focused window is a dialog or carries a sheet,
    /// and nothing for every other press, which then runs unchanged. A surface
    /// that cannot be read is treated as absent, so a slow read never blocks a
    /// key that worked before; only an exhausted deadline fails the press.
    pub(super) fn capture(
        app: &AXElement,
        pid: i32,
        action: &'static str,
        deadline: Deadline,
    ) -> Result<Option<Self>, AdapterError> {
        match Self::read(app, pid, action, deadline) {
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
        pid: i32,
        action: &'static str,
        deadline: Deadline,
    ) -> Result<Option<Self>, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let Some((surface, kind)) = modal_surface(app, instant)? else {
            return Ok(None);
        };
        let (attribute, key) = button_attribute(action);
        let present = crate::tree::surface_read::element(&surface, attribute, instant)?.is_some();
        Ok(Some(Self {
            surface,
            kind,
            action,
            pid,
            button: (key, present),
            windows: window_set(pid)?,
        }))
    }

    /// Waits a bounded time for the surface to close or the application's
    /// windows to change. `edited` also counts a change in the value of the
    /// element that held focus when the key was sent, as Return in a
    /// multi-line field does.
    pub(super) fn settle(
        &self,
        edited: Option<&FocusedValue>,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        let limit = Instant::now() + deadline.capped(SETTLE_LIMIT).remaining();
        loop {
            if self.surface_closed(deadline)?
                || window_set(self.pid)? != self.windows
                || edited.is_some_and(|value| value.changed(deadline))
            {
                return Ok(true);
            }
            if Instant::now() >= limit {
                return Ok(false);
            }
            let pause = deadline.remaining_slice(POLL_INTERVAL)?;
            std::thread::sleep(pause.min(POLL_INTERVAL));
        }
    }

    pub(super) fn verified(&self, steps: Vec<ActionStep>) -> ActionResult {
        let observed_after = steps.last().map(|step| step.label().to_string());
        ActionResult::delivered_unverified("press_key")
            .with_steps(steps)
            .with_verified_delivery()
            .with_details(serde_json::json!({
                "modal_key": {
                    "surface": self.kind,
                    "effect": "observed",
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
                "The {kind} stayed open: the key was delivered, but neither the {kind} nor any window changed",
                kind = self.kind
            ),
        )
        .with_details(serde_json::json!({ "modal_key": report }))
        .with_disposition(DeliverySemantics::delivered_unverified())
        .with_suggestion(ignored_suggestion(self.kind, self.button))
    }

    fn surface_closed(&self, deadline: Deadline) -> Result<bool, AdapterError> {
        let role = crate::tree::properties::attributes::copy_string_attr_result(
            &self.surface,
            "AXRole",
            deadline,
        );
        deadline.ensure_remaining()?;
        Ok(surface_read_shows_closed(role))
    }
}

/// The routes a confirm or cancel key takes on a sheet or dialog.
pub(super) trait ModalRoutes {
    /// Performs the semantic action; `None` when the focused element does not
    /// advertise it, so nothing was delivered.
    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError>;
    fn physical(&mut self) -> Result<ActionStep, AdapterError>;
    fn effect_observed(&mut self) -> Result<bool, AdapterError>;
}

/// Runs the semantic action, then a real key when the surface showed no
/// effect, and returns whether an effect was observed with the steps taken.
/// A semantic action that succeeded is never an effect on its own.
pub(super) fn run_routes(
    routes: &mut impl ModalRoutes,
) -> Result<(bool, Vec<ActionStep>), AdapterError> {
    let mut steps = Vec::new();
    if let Some(step) = routes.semantic()? {
        steps.push(step);
        if routes.effect_observed().map_err(after_delivery)? {
            return Ok((true, steps));
        }
    }
    let step = routes.physical().map_err(|error| {
        crate::actions::DeliveryTracker::from_delivered_units(steps.len()).annotate(error)
    })?;
    steps.push(step);
    let observed = routes.effect_observed().map_err(after_delivery)?;
    Ok((observed, steps))
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

/// The element that held focus just before a key was sent, with its value.
pub(super) struct FocusedValue {
    element: AXElement,
    value: Option<String>,
}

impl FocusedValue {
    pub(super) fn read(app: &AXElement, deadline: Deadline) -> Option<Self> {
        let element = super::focused_element(app, deadline).ok().flatten()?;
        let value = crate::tree::copy_value_typed(&element, deadline);
        Some(Self { element, value })
    }

    fn changed(&self, deadline: Deadline) -> bool {
        crate::tree::copy_value_typed(&self.element, deadline) != self.value
    }
}

fn modal_surface(
    app: &AXElement,
    deadline: Instant,
) -> Result<Option<(AXElement, &'static str)>, AdapterError> {
    use crate::tree::surface_read;

    let Some(window) = surface_read::element(app, "AXFocusedWindow", deadline)? else {
        return Ok(None);
    };
    let role = surface_read::string(&window, "AXRole", deadline)?;
    let subrole = surface_read::string(&window, "AXSubrole", deadline)?;
    if let Some(kind) = modal_kind(role.as_deref(), subrole.as_deref()) {
        return Ok(Some((window, kind)));
    }
    for child in surface_read::elements(&window, "AXChildren", deadline)? {
        if surface_read::string(&child, "AXRole", deadline)?.as_deref() == Some("AXSheet") {
            return Ok(Some((child, "sheet")));
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

fn surface_read_shows_closed(role: Result<Option<String>, i32>) -> bool {
    match role {
        Ok(role) => role.is_none(),
        Err(error) => error == accessibility_sys::kAXErrorInvalidUIElement,
    }
}

fn window_set(pid: i32) -> Result<Vec<WindowRecord>, AdapterError> {
    crate::system::window::cg_window::records_from_dictionaries(
        crate::system::window::cg_window::window_dictionaries()?,
        WindowRecordScope::Pid(pid),
    )
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
