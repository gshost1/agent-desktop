use std::cell::Cell;

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

thread_local! {
    static OPERATION_HOLDS_LEASE: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread's running FFI operation as holding the interaction lease until it is dropped.
pub(crate) struct LeaseMarker {
    _private: (),
}

impl LeaseMarker {
    pub(crate) fn hold() -> Self {
        OPERATION_HOLDS_LEASE.set(true);
        Self { _private: () }
    }
}

impl Drop for LeaseMarker {
    fn drop(&mut self) {
        OPERATION_HOLDS_LEASE.set(false);
    }
}

/// Refuses a mutation started from a log callback while this thread's running FFI operation holds the
/// interaction lease, which only this thread can release once the callback returns.
pub(crate) fn refuse_reentrant_mutation() -> Result<(), AdapterError> {
    if crate::log_callback::in_callback() && OPERATION_HOLDS_LEASE.get() {
        return Err(reentrant());
    }
    Ok(())
}

/// Marks a mutation whose lease core takes internally. Core offers no hook at the moment it acquires,
/// so the mark covers the whole call, including the work before the lease is taken.
pub(crate) fn mark_core_mutation() -> Result<LeaseMarker, AdapterError> {
    refuse_reentrant_mutation()?;
    Ok(LeaseMarker::hold())
}

fn reentrant() -> AdapterError {
    AdapterError::new(
        ErrorCode::PolicyDenied,
        "A log callback cannot start an interaction while the call that logged holds the interaction lease",
    )
    .with_suggestion(
        "Return from the log callback and make the call after the running ad_* call returns, \
         or queue it for another thread that does not wait on this one",
    )
    .with_details(serde_json::json!({
        "kind": "reentrant_interaction_lease",
        "retryable": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}
