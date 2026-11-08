//! The routes a confirm or cancel key takes on a sheet or dialog. A real key
//! goes first: a semantic `AXConfirm` on a text field ends editing and moves
//! focus off the field, after which the sheet ignores Return, while the same
//! key sent before any action reaches the field's own Return handling. On
//! Finder's Go to Folder a real Return after `set-value` navigated in 3 of 3
//! runs and the same Return after `AXConfirm` in 0 of 3. The semantic action runs only when the real key
//! could not be sent or the surface showed no reaction, and a press the
//! surface ignored after both fails instead of reporting success.

use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, InteractionPolicy,
    KeyCombo,
};

use super::modal::{FocusedValue, ModalWatch};
use super::target::KeyTarget;

pub(super) trait ModalRoutes {
    fn physical(&mut self) -> Result<ActionStep, AdapterError>;
    /// Performs the semantic action; `None` when the focused element does not
    /// advertise it, so nothing was delivered.
    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError>;
    fn effect(&mut self) -> Result<Option<&'static str>, AdapterError>;
}

/// Returns the signal that showed the surface reacted, if any, with the steps
/// taken. A delivered step that the surface did not react to is never an
/// effect on its own.
pub(super) fn run_routes(
    routes: &mut impl ModalRoutes,
) -> Result<(Option<&'static str>, Vec<ActionStep>), AdapterError> {
    let mut steps = Vec::new();
    let refused = match routes.physical() {
        Ok(step) => {
            steps.push(step);
            if let Some(signal) = routes.effect().map_err(after_delivery)? {
                return Ok((Some(signal), steps));
            }
            None
        }
        Err(error) if sent_nothing(&error) => {
            tracing::debug!(code = ?error.code, "keyboard: real key refused on a modal surface");
            Some(error)
        }
        Err(error) => return Err(error),
    };
    let delivered = crate::actions::DeliveryTracker::from_delivered_units(steps.len());
    match routes
        .semantic()
        .map_err(|error| delivered.annotate(error))?
    {
        Some(step) => steps.push(step),
        None => {
            if let Some(error) = refused {
                return Err(error);
            }
            return Ok((None, steps));
        }
    }
    let signal = routes.effect().map_err(after_delivery)?;
    Ok((signal, steps))
}

/// Sends the real key, or the semantic action when the key could not be sent,
/// for a surface whose reaction cannot be observed.
fn first_delivered_step(routes: &mut impl ModalRoutes) -> Result<ActionStep, AdapterError> {
    match routes.physical() {
        Ok(step) => Ok(step),
        Err(error) if sent_nothing(&error) => routes.semantic()?.ok_or(error),
        Err(error) => Err(error),
    }
}

fn sent_nothing(error: &AdapterError) -> bool {
    matches!(
        error.disposition,
        DeliverySemantics::Unknown | DeliverySemantics::NotDelivered
    )
}

/// The real key goes to the pid `target` picks, the same pid a press off a
/// sheet would reach.
pub(super) fn press_on_modal(
    target: &KeyTarget<'_>,
    key: (&KeyCombo, InteractionPolicy),
    modal: &ModalWatch,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let process = target.process;
    let mut routes = LiveModalRoutes {
        target,
        key,
        modal,
        edited: None,
        deadline,
    };
    if !modal.observable() {
        let step = first_delivered_step(&mut routes)?;
        crate::system::process::process_identity::require_core(process).map_err(after_delivery)?;
        return Ok(modal.unobservable(vec![step]));
    }
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
    target: &'a KeyTarget<'a>,
    key: (&'a KeyCombo, InteractionPolicy),
    modal: &'a ModalWatch,
    edited: Option<FocusedValue>,
    deadline: Deadline,
}

impl ModalRoutes for LiveModalRoutes<'_> {
    fn physical(&mut self) -> Result<ActionStep, AdapterError> {
        self.edited = FocusedValue::read(self.target.app, self.deadline);
        super::send_physical_key(self.target, self.key, self.deadline, false).map(|(step, _)| step)
    }

    fn semantic(&mut self) -> Result<Option<ActionStep>, AdapterError> {
        let action = self.modal.action();
        self.edited = None;
        Ok(
            super::try_simple_key_action(self.target.app, action, self.deadline)?
                .then(|| super::semantic_step(action)),
        )
    }

    fn effect(&mut self) -> Result<Option<&'static str>, AdapterError> {
        self.modal.settle(self.edited.as_ref(), self.deadline)
    }
}

#[cfg(test)]
#[path = "key_dispatch_modal_route_tests.rs"]
mod tests;
