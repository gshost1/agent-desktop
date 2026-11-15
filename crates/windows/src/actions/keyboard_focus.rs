//! Gives a ref-addressed keyboard target keyboard focus before keystrokes.
//!
//! Headed `type` and element-scoped `press` raise the target's window, but a
//! raised window hands keyboard focus to whichever of its controls held it
//! last, not to the target. The keystrokes are synthesized with `SendInput`,
//! which delivers to the focused control, so without this step a headed
//! `type` into a field that did not already hold focus was refused as
//! `ACTION_FAILED` on every application. When the target does not hold
//! keyboard focus, UI Automation `SetFocus` is asked once, and focus is
//! re-checked until it lands, the settle window ends or the deadline does.
//! Keystrokes are still sent only once the focus check passes.

use agent_desktop_core::{ActionStep, AdapterError, Deadline, StepMechanism};
use std::time::Duration;

const SET_FOCUS_LABEL: &str = "UIElement.SetFocus";
const FOCUS_SETTLE: Duration = Duration::from_millis(500);
const FOCUS_POLL: Duration = Duration::from_millis(20);

/// Where keyboard focus stands once the target was given the chance to take it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyboardFocus {
    AlreadyFocused,
    FocusedBySetFocus,
    NotFocused,
}

impl KeyboardFocus {
    pub(crate) fn is_ready(self) -> bool {
        !matches!(self, Self::NotFocused)
    }

    /// The step that reports a focus this command moved, so the result says
    /// that the field was focused before the keystrokes went in.
    pub(crate) fn steps(self) -> Vec<ActionStep> {
        match self {
            Self::FocusedBySetFocus => vec![
                ActionStep::succeeded(SET_FOCUS_LABEL)
                    .with_mechanism(StepMechanism::SemanticApi)
                    .with_verified(true),
            ],
            Self::AlreadyFocused | Self::NotFocused => Vec::new(),
        }
    }
}

/// The focus decision with its two live reads injected: `request_focus` asks
/// the provider to focus the target, `focus_ready` reports whether it holds
/// keyboard focus. A failed `request_focus` is not an error here: the target
/// stays unfocused and the caller refuses the keystrokes as before.
pub(crate) fn establish_with(
    deadline: Deadline,
    request_focus: impl FnOnce() -> Result<(), AdapterError>,
    mut focus_ready: impl FnMut(Deadline) -> Result<bool, AdapterError>,
) -> Result<KeyboardFocus, AdapterError> {
    if focus_ready(deadline)? {
        return Ok(KeyboardFocus::AlreadyFocused);
    }
    deadline.ensure_remaining()?;
    if request_focus().is_err() {
        return Ok(KeyboardFocus::NotFocused);
    }
    let settle = Deadline::from_duration(deadline.remaining().min(FOCUS_SETTLE))?;
    loop {
        if focus_ready(deadline)? {
            return Ok(KeyboardFocus::FocusedBySetFocus);
        }
        if settle.is_expired() {
            return Ok(KeyboardFocus::NotFocused);
        }
        std::thread::sleep(settle.remaining().min(FOCUS_POLL));
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn establish_keyboard_focus(
    element: &crate::tree::element::UIAElement,
    deadline: Deadline,
) -> Result<KeyboardFocus, AdapterError> {
    establish_with(
        deadline,
        || {
            element
                .0
                .set_focus()
                .map_err(|error| AdapterError::internal(error.to_string()))
        },
        |deadline| {
            crate::actions::physical_target::ensure_keyboard_delivery_ready(element, deadline)
        },
    )
}

#[cfg(test)]
#[path = "keyboard_focus_tests.rs"]
mod tests;
