//! The answer for a command the daemon accepted and then stopped answering for. It may have
//! acted, so it is never run again; the caller learns that delivery is uncertain and checks
//! before it retries.

use agent_desktop_core::{AdapterError, AppError, DeliverySemantics, ErrorCode};

/// The structured error for an accepted command whose daemon went away before it finished.
pub(crate) fn daemon_stopped_mid_command() -> AppError {
    AdapterError::new(
        ErrorCode::Internal,
        "The agent-desktop daemon stopped while it was running this command",
    )
    .with_suggestion(
        "The command may have taken effect. Take a fresh snapshot to check before you retry it.",
    )
    .with_disposition(DeliverySemantics::uncertain())
    .into()
}

#[cfg(test)]
#[path = "in_doubt_tests.rs"]
mod tests;
