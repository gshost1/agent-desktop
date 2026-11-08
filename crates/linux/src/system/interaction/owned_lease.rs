//! An interaction lease this process took itself, as opposed to one it adopted from its parent.

use agent_desktop_core::InteractionLease;

/// Proof that this process acquired the interaction lock itself. Only `interaction_lease::acquire` makes
/// one, and only when no lease was inherited, so code that must not run beside a sibling sharing an
/// inherited lease, such as the stale-input sweep, takes this type instead of a bare `InteractionLease`.
pub(crate) struct OwnedLease(InteractionLease);

impl OwnedLease {
    pub(super) fn new(lease: InteractionLease) -> Self {
        Self(lease)
    }

    pub(crate) fn into_lease(self) -> InteractionLease {
        self.0
    }
}
