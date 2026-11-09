use std::ops::Deref;

use agent_desktop_core::{AdapterError, InteractionLease};

use crate::lease_marker::{LeaseMarker, refuse_reentrant_mutation};

/// An interaction lease taken by an FFI operation, with this thread marked as holding it for as long as
/// the lease lives.
pub(crate) struct FfiLease {
    lease: InteractionLease,
    _marker: LeaseMarker,
}

impl FfiLease {
    /// Takes the lease with `acquire` and marks this thread once it is held, so a thread still waiting for
    /// another thread's lease is not marked. A reentrant mutation is refused before `acquire` runs.
    pub(crate) fn take(
        acquire: impl FnOnce() -> Result<InteractionLease, AdapterError>,
    ) -> Result<Self, AdapterError> {
        refuse_reentrant_mutation()?;
        let lease = acquire()?;
        Ok(Self {
            lease,
            _marker: LeaseMarker::hold(),
        })
    }
}

impl Deref for FfiLease {
    type Target = InteractionLease;

    fn deref(&self) -> &InteractionLease {
        &self.lease
    }
}
