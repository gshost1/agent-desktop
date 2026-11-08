use super::LocatorResolution;
use crate::{AppError, refs::RefMap};

/// Publishes what an incomplete walk observed, without claiming more than it
/// read.
///
/// No capture proof can vouch for a node of an incomplete observation, so a
/// match gets a ref only when its entry carries a genuine element token, which
/// the platform re-identifies by itself, and the walk read every field the ref
/// and the response need. Every other match is returned without a ref.
pub(super) fn publish_observed_matches(resolution: &mut LocatorResolution) -> Result<(), AppError> {
    let mut refmap = RefMap::new();
    for matched in &mut resolution.matches {
        matched.data.ref_id = None;
        let token_backed = crate::refs::ref_identity::element_token(&matched.entry).is_some();
        if token_backed && matched.fully_observed && matched.data.interactive {
            matched.data.ref_id = Some(refmap.try_allocate(matched.entry.clone())?);
        }
    }
    resolution.refmap = (!refmap.is_empty()).then_some(refmap);
    Ok(())
}

#[cfg(test)]
#[path = "partial_tests.rs"]
mod tests;
