//! Actions an adapter delivers through its own protocol to a target that has
//! no screen position, such as a web element driven over a browser protocol.

use super::evidence::ActionabilityEvidence;
use crate::adapter::PlatformAdapter;
use crate::{ActionRequest, NativeHandle};

/// A strictly headless request on a proven nonspatial target whose adapter
/// delivers the action without a pointer. Such a request needs neither bounds
/// nor a hit test; identity, live state and the other gates still apply.
pub(super) fn applies(
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
) -> bool {
    request.policy == crate::InteractionPolicy::headless()
        && evidence.nonspatial
        && hit_test.is_some_and(|(handle, adapter)| {
            adapter.delivers_action_without_pointer(handle, &request.action)
        })
}

#[cfg(test)]
#[path = "protocol_delivery_tests.rs"]
mod tests;
