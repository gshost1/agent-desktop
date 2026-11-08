use std::ops::Deref;

use agent_desktop_core::{AdapterError, InteractionLease};

use crate::lease_marker::LeaseMarker;

/// An interaction lease taken by an FFI operation, with this thread marked as holding it for as long as
/// the lease lives.
pub(crate) struct FfiLease {
    lease: InteractionLease,
    _marker: LeaseMarker,
}

impl FfiLease {
    /// Marks this thread, then takes the lease with `acquire`. A refused mark returns before `acquire` runs.
    pub(crate) fn take(
        acquire: impl FnOnce() -> Result<InteractionLease, AdapterError>,
    ) -> Result<Self, AdapterError> {
        let marker = LeaseMarker::enter()?;
        Ok(Self {
            lease: acquire()?,
            _marker: marker,
        })
    }
}

impl Deref for FfiLease {
    type Target = InteractionLease;

    fn deref(&self) -> &InteractionLease {
        &self.lease
    }
}
