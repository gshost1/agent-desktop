//! The routes a confirm or cancel key takes on a sheet or dialog. The
//! semantic action runs first. When the surface shows no reaction, focus goes
//! back to the control that held it, because the action can end editing and
//! move focus elsewhere in the sheet, and a real key follows. A press that the
//! surface ignored after both routes fails instead of reporting success.

use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, InteractionPolicy,
    KeyCombo, ProcessIdentity, StepMechanism,
};
use std::time::{Duration, Instant};

use super::modal::{FocusedValue, ModalWatch};
use crate::tree::AXElement;

const FOCUS_RESTORE_LIMIT: Duration = Duration::from_millis(250);
const FOCUS_POLL: Duration = Duration::from_millis(10);

pub(super) trait ModalRoutes {
    /// Performs the semantic action; `None` when the focused element does not
    /// advertise it, so nothing was delivered.
    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError>;
    /// Puts focus back on the control that held it before the semantic
    /// action; `None` when focus never moved.
    fn restore_focus(&mut self) -> Result<Option<ActionStep>, AdapterError>;
    fn physical(&mut self) -> Result<ActionStep, AdapterError>;
    fn effect(&mut self) -> Result<Option<&'static str>, AdapterError>;
}

/// Returns the signal that showed the surface reacted, if any, with the steps
/// taken. A semantic action that succeeded is never a reaction on its own.
pub(super) fn run_routes(
    routes: &mut impl ModalRoutes,
) -> Result<(Option<&'static str>, Vec<ActionStep>), AdapterError> {
    let mut steps = Vec::new();
    if let Some(step) = routes.semantic()? {
        steps.push(step);
        if let Some(signal) = routes.effect().map_err(after_delivery)? {
            return Ok((Some(signal), steps));
        }
        if let Some(step) = routes.restore_focus().map_err(after_delivery)? {
            steps.push(step);
        }
    }
    let step = routes.physical().map_err(|error| {
        crate::actions::DeliveryTracker::from_delivered_units(steps.len()).annotate(error)
    })?;
    steps.push(step);
    let signal = routes.effect().map_err(after_delivery)?;
    Ok((signal, steps))
}

pub(super) fn press_on_modal(
    process: &ProcessIdentity,
    app: &AXElement,
    key: (&KeyCombo, InteractionPolicy),
    modal: &ModalWatch,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let mut routes = LiveModalRoutes {
        process,
        app,
        key,
        modal,
        focused_before: None,
        edited: None,
        deadline,
    };
    let (signal, mut steps) = run_routes(&mut routes)?;
    let Some(signal) = signal else {
        return Err(modal.ignored(&steps));
    };
    tracing::debug!(signal, "keyboard: modal surface reacted");
    crate::system::process::process_identity::require_core(process).map_err(after_delivery)?;
    if let Some(last) = steps.pop() {
        steps.push(last.with_verified(true));
    }
    Ok(modal.verified(steps, signal))
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

struct LiveModalRoutes<'a> {
    process: &'a ProcessIdentity,
    app: &'a AXElement,
    key: (&'a KeyCombo, InteractionPolicy),
    modal: &'a ModalWatch,
    focused_before: Option<AXElement>,
    edited: Option<FocusedValue>,
    deadline: Deadline,
}

impl ModalRoutes for LiveModalRoutes<'_> {
    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError> {
        let action = self.modal.action();
        self.focused_before = super::focused_element(self.app, self.deadline)
            .ok()
            .flatten();
        Ok(
            super::try_simple_key_action(self.app, action, self.deadline)?
                .then(|| super::semantic_step(action)),
        )
    }

    fn restore_focus(&mut self) -> Result<Option<ActionStep>, AdapterError> {
        let Some(before) = &self.focused_before else {
            return Ok(None);
        };
        if focus_is_on(self.app, before, self.deadline) {
            return Ok(None);
        }
        let focused = crate::actions::ax_helpers::ax_focus_or_err(before, self.deadline)
            .unwrap_or(false)
            && wait_for_focus(self.app, before, self.deadline)?;
        tracing::debug!(focused, "keyboard: focus restored before the real key");
        Ok(Some(if focused {
            ActionStep::succeeded("AXFocused")
                .with_mechanism(StepMechanism::SemanticApi)
                .with_verified(true)
        } else {
            ActionStep::skipped("AXFocused")
        }))
    }

    fn physical(&mut self) -> Result<ActionStep, AdapterError> {
        self.edited = FocusedValue::read(self.app, self.deadline);
        let (combo, policy) = self.key;
        super::send_physical_key(self.process, self.app, combo, policy, self.deadline)
    }

    fn effect(&mut self) -> Result<Option<&'static str>, AdapterError> {
        self.modal.settle(self.edited.as_ref(), self.deadline)
    }
}

fn focus_is_on(app: &AXElement, element: &AXElement, deadline: Deadline) -> bool {
    super::focused_element(app, deadline)
        .ok()
        .flatten()
        .is_some_and(|focused| crate::tree::same_element(&focused, element))
}

fn wait_for_focus(
    app: &AXElement,
    element: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let limit = Instant::now() + deadline.capped(FOCUS_RESTORE_LIMIT).remaining();
    loop {
        if focus_is_on(app, element, deadline) {
            return Ok(true);
        }
        if Instant::now() >= limit {
            return Ok(false);
        }
        let pause = deadline.remaining_slice(FOCUS_POLL)?;
        std::thread::sleep(pause.min(FOCUS_POLL));
    }
}

#[cfg(test)]
#[path = "key_dispatch_modal_route_tests.rs"]
mod tests;
