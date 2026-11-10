//! The `--first` match whose ref only a whole-window read can prove.

use super::{LocatorResolution, ObservedTree};

/// Whether a selected match would get a ref with no element token that its
/// capture left incomplete. Only a read of the whole window can prove such a
/// ref unique; an ambiguous or unknown verdict stays as it is with a longer
/// read, so it does not qualify.
pub(super) fn has_unproven_tokenless_match(
    resolution: &LocatorResolution,
    tree: &ObservedTree,
) -> bool {
    resolution.matches.iter().any(|found| {
        found.entry.identity.capture == crate::CaptureVerdict::Incomplete
            && tree
                .nodes
                .get(found.document_order as usize)
                .is_some_and(super::materialize::may_publish_tokenless_ref)
    })
}
