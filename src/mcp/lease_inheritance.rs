//! Whether this server runs under an interaction lease its parent holds, resolved once when it starts.

use clap::Parser;
use serde_json::Value;
use std::thread::JoinHandle;

use super::child_lease::ChildLease;
use super::inherited_lease::InheritedLease;
use super::lease_slot::Ticket;
use crate::cli::{Cli, Commands};

#[cfg(unix)]
const LEASE_VARIABLE: Option<&str> = Some(agent_desktop_core::INTERACTION_LEASE_FD_ENV);
#[cfg(windows)]
const LEASE_VARIABLE: Option<&str> = Some(agent_desktop_core::INTERACTION_LEASE_HANDLE_ENV);
#[cfg(not(any(unix, windows)))]
const LEASE_VARIABLE: Option<&str> = None;

/// `Owned` when no lease was inherited: every child takes the lease itself, as a CLI run does. `Inherited`
/// when the parent handed its lease to this server: children act under it one at a time. `Unusable` when
/// the variable is set but names no open descriptor above the standard streams (on Windows, no integer
/// handle): no child sees it, so each takes the real lock itself, which stays exclusive whoever holds it.
pub(crate) enum LeaseInheritance {
    Owned,
    Inherited(Box<InheritedLease>),
    Unusable(&'static str),
}

impl LeaseInheritance {
    pub(crate) fn from_env() -> Self {
        let Some((variable, value)) = LEASE_VARIABLE
            .and_then(|variable| std::env::var_os(variable).map(|value| (variable, value)))
        else {
            return Self::Owned;
        };
        #[cfg(unix)]
        let inherited = value
            .to_str()
            .and_then(agent_desktop_core::protect_inherited_lease_descriptor)
            .map(|descriptor| {
                InheritedLease::new(variable, value.clone()).with_descriptor(descriptor)
            });
        #[cfg(not(unix))]
        let inherited = value
            .to_str()
            .and_then(|handle| handle.parse::<isize>().ok())
            .map(|_| InheritedLease::new(variable, value.clone()));
        match inherited {
            Some(lease) => Self::Inherited(Box::new(lease)),
            None => {
                eprintln!(
                    "agent-desktop mcp: ignoring {variable}, which names no usable inherited lease; tool calls take the lease themselves"
                );
                Self::Unusable(variable)
            }
        }
    }

    /// Puts admitted request `id` in line for the lease; `None` unless a lease was inherited.
    pub(crate) fn register(&self, id: &Value) -> Option<Ticket> {
        match self {
            Self::Inherited(lease) => Some(lease.register(&id.to_string())),
            Self::Owned | Self::Unusable(_) => None,
        }
    }

    /// What the child of the call holding `ticket` receives. Under an inherited lease, a call for which
    /// `needs_lease` is true waits for the slot; `None` when it was cancelled first, so it must not spawn.
    pub(crate) fn child_lease(
        &self,
        ticket: Option<Ticket>,
        needs_lease: impl FnOnce() -> bool,
    ) -> Option<ChildLease<'_>> {
        match (self, ticket) {
            (Self::Owned, _) => Some(ChildLease::Unchanged),
            (Self::Unusable(variable), _) => Some(ChildLease::Withheld(variable)),
            (Self::Inherited(lease), Some(ticket)) if needs_lease() => lease.lend_to(ticket),
            (Self::Inherited(lease), ticket) => {
                if let Some(ticket) = ticket {
                    lease.forget(ticket);
                }
                Some(ChildLease::Withheld(lease.variable()))
            }
        }
    }

    pub(crate) fn forget(&self, ticket: Option<Ticket>) {
        if let (Self::Inherited(lease), Some(ticket)) = (self, ticket) {
            lease.forget(ticket);
        }
    }

    /// Cancels request `id` through `cancel_call`, which returns the thread reaping its child, if any.
    pub(crate) fn cancel(&self, id: &Value, cancel_call: impl FnOnce() -> Option<JoinHandle<()>>) {
        match self {
            Self::Inherited(lease) => lease.cancel(&id.to_string(), cancel_call),
            Self::Owned | Self::Unusable(_) => {
                let _ = cancel_call();
            }
        }
    }
}

/// True for a command that changes something, or that reads the permission report (`status`,
/// `permissions`), which can release stuck keys under the lease. Read-only commands, accessibility reads
/// included, run beside the holder without the lease, so a `wait` never blocks the action it waits for.
/// Anything that does not parse counts as needing the lease.
pub(crate) fn needs_lease(globals: &[String], argv: &[String]) -> bool {
    let words = std::iter::once("agent-desktop")
        .chain(globals.iter().map(String::as_str))
        .chain(argv.iter().map(String::as_str));
    match Cli::try_parse_from(words) {
        Ok(Cli {
            command: Some(command),
            ..
        }) => {
            command.is_mutating() || matches!(command, Commands::Status | Commands::Permissions(_))
        }
        _ => true,
    }
}

#[cfg(test)]
#[path = "lease_inheritance_tests.rs"]
mod tests;
