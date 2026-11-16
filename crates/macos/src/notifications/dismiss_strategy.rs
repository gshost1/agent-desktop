//! The order in which a notification is dismissed, and what each failure says
//! about delivery.
//!
//! Every strategy that sends, or may have sent, a dismissal is counted, and
//! any later error (a re-read, a preflight, a policy refusal or the final
//! "all strategies failed") carries an unsafe disposition once one has: the
//! notification may already be gone, so a retry could dismiss whichever
//! notification now holds its index. A read's own `not_delivered` never
//! overrides that.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

use crate::actions::DeliveryTracker;

/// The notification-side steps a dismissal is built from.
pub(super) trait DismissOps {
    /// Performs a native accessibility action on the notification.
    fn perform(&self, action: &str) -> Result<bool, AdapterError>;

    /// Presses a close, clear or dismiss button among the notification's
    /// children, counting every press that may have been delivered.
    fn press_close_button(&self, delivery: &mut DeliveryTracker) -> Result<bool, AdapterError>;

    /// Re-reads the list and reports whether the notification is gone.
    fn is_gone(&self) -> Result<bool, AdapterError>;

    /// Refuses before the pointer is used to reveal the close button.
    fn may_reveal(&self) -> Result<(), AdapterError>;

    /// Moves the pointer over the notification to reveal its close button.
    fn hover(&self) -> Result<(), AdapterError>;
}

pub(super) fn run_dismiss(ops: &impl DismissOps, deadline: Deadline) -> Result<(), AdapterError> {
    let mut delivery = DeliveryTracker::default();
    run_strategies(ops, deadline, &mut delivery).map_err(|error| delivery.annotate(error))
}

fn run_strategies(
    ops: &impl DismissOps,
    deadline: Deadline,
    delivery: &mut DeliveryTracker,
) -> Result<(), AdapterError> {
    for action in ["AXDismiss", "AXRemoveFromParent"] {
        let outcome = ops.perform(action);
        if verified(ops, outcome, deadline, delivery)? {
            return Ok(());
        }
    }
    let pressed = ops.press_close_button(delivery);
    if verified(ops, pressed, deadline, delivery)? {
        return Ok(());
    }
    deadline.ensure_remaining()?;
    ops.may_reveal()?;
    if let Err(error) = ops.hover() {
        record(delivery, Err(&error));
        super::read::tolerate_ax_strategy_error(error, deadline)?;
        return Err(all_dismiss_strategies_failed());
    }
    delivery.mark_delivered();
    std::thread::sleep(deadline.remaining_slice(std::time::Duration::from_millis(300))?);
    let pressed = ops.press_close_button(delivery);
    if verified(ops, pressed, deadline, delivery)? {
        return Ok(());
    }
    Err(all_dismiss_strategies_failed())
}

fn verified(
    ops: &impl DismissOps,
    outcome: Result<bool, AdapterError>,
    deadline: Deadline,
    delivery: &mut DeliveryTracker,
) -> Result<bool, AdapterError> {
    record(delivery, outcome.as_ref().copied());
    super::dismiss_verify::verified_after_strategy(outcome, deadline, || match ops.is_gone() {
        Ok(gone) => Ok(gone),
        Err(error) => {
            super::read::tolerate_ax_strategy_error(error, deadline)?;
            Ok(false)
        }
    })
}

/// Counts a strategy outcome that sent, or may have sent, a dismissal.
pub(super) fn record(delivery: &mut DeliveryTracker, outcome: Result<bool, &AdapterError>) {
    let sent = match outcome {
        Ok(sent) => sent,
        Err(error) => matches!(
            error.disposition,
            DeliverySemantics::DeliveryUncertain
                | DeliverySemantics::DeliveredUnverified
                | DeliverySemantics::DeliveredVerified
        ),
    };
    if sent {
        delivery.mark_delivered();
    }
}

fn all_dismiss_strategies_failed() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "All dismiss strategies failed (AXDismiss, AXRemoveFromParent, close button, hover+close)",
    )
}

#[cfg(test)]
#[path = "dismiss_strategy_tests.rs"]
mod tests;
