//! The `--first` match whose ref only a whole-window read can prove.

use super::{LocatorResolution, ObservedTree};
use crate::{AppError, RefEntry, refs::RefMap};

/// A ref with no element token that its capture left incomplete: every
/// action refuses it, and only a read of the whole window can prove it.
fn unproven_tokenless(entry: &RefEntry) -> bool {
    entry.identity.capture == crate::CaptureVerdict::Incomplete
        && crate::refs::ref_identity::element_token(entry).is_none()
}

/// Whether a selected, addressable match would get such a ref. An ambiguous
/// or unknown verdict stays as it is with a longer read, so it does not
/// qualify.
pub(super) fn has_unproven_tokenless_match(
    resolution: &LocatorResolution,
    tree: &ObservedTree,
) -> bool {
    resolution.matches.iter().any(|found| {
        unproven_tokenless(&found.entry)
            && tree
                .nodes
                .get(found.document_order as usize)
                .is_some_and(|node| super::materialize::addressability(&node.evidence).0)
    })
}

/// Keeps the hydrated matches and drops every ref that is still unproven,
/// so a `--first` answer never publishes a ref every action would refuse.
pub(super) fn withhold_unproven_refs(resolution: &mut LocatorResolution) -> Result<(), AppError> {
    let mut refmap = RefMap::new();
    for found in &mut resolution.matches {
        if found.data.ref_id.is_none() {
            continue;
        }
        found.data.ref_id = if unproven_tokenless(&found.entry) {
            None
        } else {
            Some(refmap.try_allocate(found.entry.clone())?)
        };
    }
    resolution.refmap = Some(refmap);
    Ok(())
}

/// A row pass that runs out of time or finds the application not answering
/// leaves the match the first pass found standing; a closed window, a revoked
/// permission or any other failure is returned as it is.
pub(super) fn kept_through_row_pass_failure(error: &AppError) -> bool {
    [crate::ErrorCode::Timeout, crate::ErrorCode::AppUnresponsive]
        .iter()
        .any(|code| error.code() == code.as_str())
}
