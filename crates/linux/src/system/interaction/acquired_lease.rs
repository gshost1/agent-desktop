//! How this process came to hold the interaction lease.

use agent_desktop_core::InteractionLease;

use super::owned_lease::OwnedLease;

/// `Owned` when this process took the interaction lock itself. `Adopted` when it adopted a lease its parent
/// passed down: every process that adopts that descriptor shares one open file description, so the lock
/// cannot tell this process from a sibling acting under the same lease at the same moment.
pub(crate) enum AcquiredLease {
    Owned(OwnedLease),
    Adopted(InteractionLease),
}

impl AcquiredLease {
    /// The lease for work that only needs the lock held, whoever holds it.
    pub(crate) fn into_lease(self) -> InteractionLease {
        match self {
            Self::Owned(lease) => lease.into_lease(),
            Self::Adopted(lease) => lease,
        }
    }

    /// The owned-lease proof; `None` for an adopted lease.
    pub(crate) fn owned(self) -> Option<OwnedLease> {
        match self {
            Self::Owned(lease) => Some(lease),
            Self::Adopted(_) => None,
        }
    }
}
