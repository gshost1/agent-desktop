//! Whether this server runs under an interaction lease its parent holds, resolved once when it starts.

use clap::Parser;
use serde_json::Value;

use super::child_lease::ChildLease;
use super::inherited_lease::InheritedLease;
use crate::cli::Cli;
use crate::command_policy::{PermissionNeed, policy_for};

/// The variable a lease-holding parent sets to hand its interaction lease to a child.
#[cfg(unix)]
pub(crate) const LEASE_VARIABLE: Option<&str> = Some(agent_desktop_core::INTERACTION_LEASE_FD_ENV);
/// The Windows adapter's `INTERACTION_LEASE_HANDLE_ENV`, private to that crate.
#[cfg(windows)]
pub(crate) const LEASE_VARIABLE: Option<&str> = Some("AGENT_DESKTOP_INTERACTION_LEASE_HANDLE");
#[cfg(not(any(unix, windows)))]
pub(crate) const LEASE_VARIABLE: Option<&str> = None;

/// `Owned` when no lease was inherited: every child takes the lease itself, as a CLI run does. `Inherited`
/// when the parent handed its lease to this server: children act under it one at a time.
pub(crate) enum LeaseInheritance {
    Owned,
    Inherited(InheritedLease),
}

impl LeaseInheritance {
    pub(crate) fn from_env() -> Self {
        let inherited = LEASE_VARIABLE
            .and_then(|variable| std::env::var_os(variable).map(|value| (variable, value)));
        match inherited {
            None => Self::Owned,
            Some((variable, value)) => Self::Inherited(InheritedLease::new(variable, value)),
        }
    }

    /// What the child of call `id` receives. Under an inherited lease, a call for which `needs_lease` is true
    /// waits for the slot; `None` when it was cancelled while waiting, so it must not spawn.
    pub(crate) fn child_lease(
        &self,
        id: &Value,
        needs_lease: impl FnOnce() -> bool,
    ) -> Option<ChildLease<'_>> {
        match self {
            Self::Owned => Some(ChildLease::Unchanged),
            Self::Inherited(lease) if needs_lease() => lease.lend_to(&id.to_string()),
            Self::Inherited(lease) => Some(ChildLease::Withheld(lease)),
        }
    }

    pub(crate) fn cancel(&self, id: &Value) {
        if let Self::Inherited(lease) = self {
            lease.cancel(&id.to_string());
        }
    }
}

/// False only for a command that neither changes anything nor needs a platform permission, the commands
/// that never take the interaction lease; anything that does not parse counts as needing it.
pub(crate) fn needs_lease(globals: &[String], argv: &[String]) -> bool {
    let words = std::iter::once("agent-desktop")
        .chain(globals.iter().map(String::as_str))
        .chain(argv.iter().map(String::as_str));
    match Cli::try_parse_from(words) {
        Ok(Cli {
            command: Some(command),
            ..
        }) => command.is_mutating() || policy_for(&command) != PermissionNeed::None,
        _ => true,
    }
}

#[cfg(test)]
#[path = "lease_inheritance_tests.rs"]
mod tests;
