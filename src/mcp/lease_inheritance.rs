//! Whether this server runs under an interaction lease its parent holds, resolved once when it starts.

use agent_desktop_core::{AdapterError, AppError, DeliverySemantics, ErrorCode};
use serde_json::Value;
use std::thread::JoinHandle;

use super::child_lease::ChildLease;
use super::inherited_lease::InheritedLease;
use super::lease_refusal::LeaseRefusal;
use super::lease_role::LeaseRole;
use super::lease_slot::Ticket;

#[cfg(unix)]
const LEASE_VARIABLE: Option<&str> = Some(agent_desktop_core::INTERACTION_LEASE_FD_ENV);
#[cfg(windows)]
const LEASE_VARIABLE: Option<&str> = Some(agent_desktop_core::INTERACTION_LEASE_HANDLE_ENV);
#[cfg(not(any(unix, windows)))]
const LEASE_VARIABLE: Option<&str> = None;

/// `Owned` when no lease was inherited: every child takes the lease itself, as a CLI run does. `Inherited`
/// when the parent handed its lease to this server: children act under it by their `LeaseRole`. `Unusable`
/// when the variable is set but names no open descriptor above the standard streams (on Windows, no integer
/// handle): the parent presumably holds the real lock, so a call that needs the lease is refused at once
/// rather than left to wait out its deadline, and the rest run without the variable.
pub(crate) enum LeaseInheritance {
    Owned,
    Inherited(Box<InheritedLease>),
    Unusable {
        variable: &'static str,
        value: String,
    },
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
            None => Self::Unusable {
                variable,
                value: value.to_string_lossy().into_owned(),
            },
        }
    }

    /// Puts an admitted call, with or without a request `id`, in line; `None` unless a lease was inherited.
    pub(crate) fn register(&self, id: Option<&Value>) -> Option<Ticket> {
        match self {
            Self::Inherited(lease) => Some(lease.register(id.map(Value::to_string).as_deref())),
            Self::Owned | Self::Unusable { .. } => None,
        }
    }

    /// What the child of the call holding `ticket` receives, by the call's `role`. An exclusive call waits
    /// for its turn; it is refused when cancelled first, and every call that needs the lease is refused when
    /// the inherited value is unusable.
    pub(crate) fn child_lease(
        &self,
        ticket: Option<Ticket>,
        role: impl FnOnce() -> LeaseRole,
    ) -> Result<ChildLease<'_>, LeaseRefusal> {
        let lease = match self {
            Self::Owned => return Ok(ChildLease::Unchanged),
            Self::Unusable { variable, value } => {
                return match role() {
                    LeaseRole::Unused => Ok(ChildLease::Withheld(variable)),
                    _ => Err(LeaseRefusal::Unusable(unusable(variable, value))),
                };
            }
            Self::Inherited(lease) => lease,
        };
        match (role(), ticket) {
            (LeaseRole::Exclusive, Some(ticket)) => {
                lease.lend_to(ticket).ok_or(LeaseRefusal::Cancelled)
            }
            (LeaseRole::Exclusive, None) => Err(LeaseRefusal::Cancelled),
            (role, ticket) => {
                self.forget(ticket);
                Ok(match role {
                    LeaseRole::Unused => ChildLease::Withheld(lease.variable()),
                    _ => ChildLease::Shared(lease),
                })
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
            Self::Owned | Self::Unusable { .. } => {
                let _ = cancel_call();
            }
        }
    }
}

fn unusable(variable: &str, value: &str) -> AppError {
    AdapterError::new(
        ErrorCode::PolicyDenied,
        format!(
            "{variable}={value} names no inherited interaction lease this server can lend, and its parent may hold the lock"
        ),
    )
    .with_suggestion(format!(
        "Start agent-desktop mcp with {variable} set to an open descriptor of the interaction lock above 2, or without it"
    ))
    .with_disposition(DeliverySemantics::not_delivered())
    .into()
}

#[cfg(test)]
#[path = "lease_inheritance_tests.rs"]
mod tests;
