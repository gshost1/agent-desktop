use std::cell::Cell;

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

thread_local! {
    static OPERATION_HOLDS_LEASE: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread's running FFI operation as holding, or about to take, the interaction lease, until it
/// is dropped. A mutating call made from a log callback while the mark is set is refused instead of waiting
/// for a lease that only the callback's own thread can release.
pub(crate) struct LeaseMarker {
    previous: bool,
}

impl LeaseMarker {
    pub(crate) fn enter() -> Result<Self, AdapterError> {
        if crate::log_callback::in_callback() && OPERATION_HOLDS_LEASE.get() {
            return Err(reentrant());
        }
        Ok(Self {
            previous: OPERATION_HOLDS_LEASE.replace(true),
        })
    }
}

impl Drop for LeaseMarker {
    fn drop(&mut self) {
        OPERATION_HOLDS_LEASE.set(self.previous);
    }
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
