//! The `--first` match that must wait for the row pass before it gets a ref.

use super::{LocatorResolution, ObservedTree};

/// Whether a selected match would get a ref with no element token whose
/// capture is incomplete only because the rows were left unread. A longer read
/// can prove that ref unique; an ambiguous or unknown verdict stays as it is,
/// so it gets no extra pass.
pub(super) fn awaits_row_proof(resolution: &LocatorResolution, tree: &ObservedTree) -> bool {
    resolution.matches.iter().any(|found| {
        found.entry.identity.capture == crate::CaptureVerdict::Incomplete
            && crate::refs::ref_identity::element_token(&found.entry).is_none()
            && tree
                .nodes
                .get(found.document_order as usize)
                .is_some_and(|node| {
                    let (addressable, settled) = super::materialize::addressability(&node.evidence);
                    addressable || !settled
                })
    })
}
