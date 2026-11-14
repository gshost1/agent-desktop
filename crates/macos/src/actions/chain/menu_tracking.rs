//! Settles a menu-opening action whose accessibility call timed out.
//!
//! `AXShowMenu` does not return until the menu it opened finishes tracking, so
//! a context menu that opened correctly still reports `kAXErrorCannotComplete`
//! once the messaging timeout runs out. The return code cannot tell that
//! apart from a hung application; reading the application's menu surface can.
//! Because that read is the proof, the call itself gets only a short messaging
//! timeout instead of waiting out the menu's whole tracking session.

use accessibility_sys::kAXErrorCannotComplete;
use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

use crate::actions::chain::chain_delivery::DeliveryOutcome;
use crate::actions::chain::menu_observation::MenuObservation;

pub(crate) const MENU_SETTLE_MS: u64 = 600;
const MENU_ACTION_BUDGET_MS: u64 = 500;
const TRACKING_SETTLE_MS: u64 = 1_500;
const MENU_POLL_MS: u64 = 25;

/// The budget for one menu-opening perform. A mutation gets half its budget as
/// the messaging timeout, so the call waits about 250 ms for a reply that a
/// tracking menu never sends, and the menu read decides what happened. The
/// caller's deadline still bounds the read that follows.
pub(crate) fn menu_action_deadline(deadline: Deadline) -> Deadline {
    deadline.capped(std::time::Duration::from_millis(MENU_ACTION_BUDGET_MS))
}

pub(crate) fn is_tracking_timeout(error: &AdapterError) -> bool {
    error.code == ErrorCode::AppUnresponsive
        && error
            .details
            .as_ref()
            .and_then(|details| details.get("ax_error"))
            .and_then(serde_json::Value::as_i64)
            == Some(i64::from(kAXErrorCannotComplete))
}

/// A menu that was positively read as closed before the action and is read
/// as open now is the postcondition, so the delivery is verified. An
/// application that keeps answering without a menu is responsive, so the
/// failure is not reported as a hang, but the timed-out call may still be
/// processed and the delivery stays uncertain. An application that never
/// answers keeps the original unresponsive failure.
pub(crate) fn settle_tracking_timeout(
    failure: AdapterError,
    baseline: MenuObservation,
    mut observe: impl FnMut() -> Option<MenuObservation>,
) -> Result<DeliveryOutcome, AdapterError> {
    let mut answered = false;
    while let Some(observation) = observe() {
        match observation {
            MenuObservation::Open if baseline == MenuObservation::Closed => {
                return Ok(DeliveryOutcome::DeliveredVerified);
            }
            MenuObservation::Open => return Err(failure),
            MenuObservation::Closed => answered = true,
            MenuObservation::Unreadable => {}
        }
    }
    if answered {
        Err(responsive_without_menu(failure))
    } else {
        Err(failure)
    }
}

fn responsive_without_menu(failure: AdapterError) -> AdapterError {
    let operation = failure
        .details
        .as_ref()
        .and_then(|details| details.get("operation"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("menu action")
        .to_string();
    let mut error = AdapterError::new(
        ErrorCode::ActionFailed,
        format!("{operation} timed out and no menu opened while the application kept answering"),
    )
    .with_details(serde_json::json!({
        "ax_error": kAXErrorCannotComplete,
        "operation": operation,
        "menu_observed": false,
        "perform_budget_ms": MENU_ACTION_BUDGET_MS,
    }))
    .with_disposition(DeliverySemantics::uncertain())
    .with_suggestion(
        "Run 'wait --menu' or take a fresh snapshot before deciding whether to retry; the timed-out request may still open the menu.",
    );
    if let Some(detail) = failure.platform_detail {
        error = error.with_platform_detail(detail);
    }
    error
}

mod imp {
    use agent_desktop_core::Deadline;

    use super::{MENU_POLL_MS, TRACKING_SETTLE_MS};
    use crate::actions::chain::menu_observation::MenuObservation;

    pub(crate) fn observe_menu(pid: i32, deadline: Deadline) -> MenuObservation {
        match crate::tree::locator_deadline::from_operation(deadline)
            .and_then(|instant| crate::tree::surfaces::is_menu_open(pid, instant))
        {
            Ok(true) => MenuObservation::Open,
            Ok(false) => MenuObservation::Closed,
            Err(_) => MenuObservation::Unreadable,
        }
    }

    /// Reads the menu surface until the bounded settle window or the
    /// operation deadline ends, pausing between reads but not before the first.
    /// The window outlasts the short perform timeout so a slow application
    /// that is still opening its menu is not reported as failed; it ends at
    /// the first read that sees the menu.
    pub(crate) fn poll_menu(
        pid: i32,
        deadline: Deadline,
    ) -> impl FnMut() -> Option<MenuObservation> {
        let settle_end =
            std::time::Instant::now() + std::time::Duration::from_millis(TRACKING_SETTLE_MS);
        let mut first = true;
        move || {
            if !first {
                if deadline.is_expired() || std::time::Instant::now() >= settle_end {
                    return None;
                }
                std::thread::sleep(
                    deadline
                        .remaining()
                        .min(std::time::Duration::from_millis(MENU_POLL_MS)),
                );
            }
            first = false;
            Some(observe_menu(pid, deadline))
        }
    }
}

pub(crate) use imp::{observe_menu, poll_menu};

#[cfg(test)]
#[path = "menu_tracking_tests.rs"]
mod tests;
