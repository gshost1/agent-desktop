//! Gives a ref-addressed keyboard target keyboard focus before headed keystrokes.
//!
//! Headed `type` and element-scoped `press` raise the target's window, but a
//! raised window hands keyboard focus to whichever of its controls held it
//! last, not to the target. The keystrokes are synthesized with `SendInput`,
//! which delivers to the focused control, so without this step a headed
//! `type` into a field that did not already hold focus was refused as
//! `ACTION_FAILED` on every application. When the target does not hold
//! keyboard focus, UI Automation `SetFocus` is asked once, and focus is
//! re-checked until it lands, the settle window ends or the deadline does.
//!
//! `SetFocus` moves the desktop foreground window (A3-4, A19-5), so it runs
//! only under a headed policy. A focus-fallback policy (the default `press
//! <ref>`, an FFI caller's focus-fallback kind) keeps the pure focus check:
//! an unfocused target is refused with no focus moved. Keystrokes are sent
//! only once the focus check passes.

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, StepMechanism,
};
use std::time::Duration;

use crate::actions::mutation::classify_mutation;
use crate::system::runtime::hresult::com_hresult_detail;
use crate::tree::automation::UiaFailure;

const SET_FOCUS_LABEL: &str = "UIElement.SetFocus";
const SET_FOCUS_OP: &str = "SetFocus";
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

    /// The command's steps in delivery order: the `SetFocus` this command
    /// ran, when it ran one, then the keystroke step.
    pub(crate) fn then(self, keystroke: ActionStep) -> Vec<ActionStep> {
        let mut steps = Vec::with_capacity(2);
        if self == Self::FocusedBySetFocus {
            steps.push(
                ActionStep::succeeded(SET_FOCUS_LABEL)
                    .with_mechanism(StepMechanism::SemanticApi)
                    .with_verified(true),
            );
        }
        steps.push(keystroke);
        steps
    }
}

/// The focus decision with its two live reads injected: `request_focus` asks
/// the provider to focus the target, `focus_ready` reports whether it holds
/// keyboard focus. `may_move_focus` is false outside headed mode, where an
/// unfocused target is reported `NotFocused` without `request_focus` being
/// called. A `request_focus` error is returned as it is, and a target that
/// accepted `SetFocus` but never reports focus is refused as not delivered.
pub(crate) fn establish_with(
    deadline: Deadline,
    may_move_focus: bool,
    request_focus: impl FnOnce() -> Result<(), AdapterError>,
    mut focus_ready: impl FnMut(Deadline) -> Result<bool, AdapterError>,
) -> Result<KeyboardFocus, AdapterError> {
    if focus_ready(deadline)? {
        return Ok(KeyboardFocus::AlreadyFocused);
    }
    if !may_move_focus {
        return Ok(KeyboardFocus::NotFocused);
    }
    deadline.ensure_remaining()?;
    request_focus()?;
    let settle = Deadline::from_duration(deadline.remaining().min(FOCUS_SETTLE))?;
    loop {
        if focus_ready(deadline)? {
            return Ok(KeyboardFocus::FocusedBySetFocus);
        }
        if settle.is_expired() {
            return Err(focus_not_taken(None));
        }
        std::thread::sleep(settle.remaining().min(FOCUS_POLL));
    }
}

/// The refusal for a failed `SetFocus`, through the same classifier every UIA
/// write uses: an element that is gone is `STALE_REF`, a denied one
/// `PERM_DENIED`, and one with no focus affordance is refused as not taking
/// focus. No keystroke has been sent, so the action is always not delivered,
/// and the HRESULT stays in `platform_detail`.
pub(crate) fn set_focus_refusal(failure: &UiaFailure) -> AdapterError {
    let error = match classify_mutation(SET_FOCUS_OP, SET_FOCUS_LABEL, failure) {
        Ok(_) => focus_not_taken(Some(failure_detail(failure))),
        Err(error) => error,
    };
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn focus_not_taken(platform_detail: Option<String>) -> AdapterError {
    let error = AdapterError::new(
        ErrorCode::ActionFailed,
        "Target did not take keyboard focus",
    )
    .with_suggestion(
        "Run 'snapshot' to check that the target is an enabled, focusable field, then retry.",
    )
    .with_disposition(DeliverySemantics::not_delivered());
    match platform_detail {
        Some(detail) => error.with_platform_detail(detail),
        None => error,
    }
}

fn failure_detail(failure: &UiaFailure) -> String {
    match *failure {
        UiaFailure::Hresult(hresult) => format!(
            "{SET_FOCUS_LABEL}({SET_FOCUS_OP}) returned {}",
            com_hresult_detail(hresult)
        ),
        UiaFailure::Sentinel(sentinel) => {
            format!("{SET_FOCUS_LABEL}({SET_FOCUS_OP}) returned sentinel {sentinel}")
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn establish_keyboard_focus(
    element: &crate::tree::element::UIAElement,
    policy: agent_desktop_core::InteractionPolicy,
    deadline: Deadline,
) -> Result<KeyboardFocus, AdapterError> {
    establish_with(
        deadline,
        policy.is_headed(),
        || {
            element
                .0
                .set_focus()
                .map_err(|error| set_focus_refusal(&crate::tree::automation::failure_of(&error)))
        },
        |deadline| {
            crate::actions::physical_target::ensure_keyboard_delivery_ready(element, deadline)
        },
    )
}

#[cfg(test)]
#[path = "keyboard_focus_tests.rs"]
mod tests;
