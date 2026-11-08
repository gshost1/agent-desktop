use super::{LocatorMatch, LocatorResolution, LocatorResolveRequest, ObservedTree};
use crate::{AppError, adapter::PlatformAdapter, locator::LocatorQuery, refs::RefMap};

/// Publishes what an incomplete walk observed, without claiming more than it
/// read.
///
/// No capture proof can vouch for a node of an incomplete observation, so only
/// a match whose entry carries a genuine element token can get a ref: the
/// platform re-identifies a token by itself. Each such match goes through the
/// same selected-match hydration a complete answer uses, which reads the fields
/// the walk skipped and checks the landed element against the match. A match
/// that hydration cannot settle, and every tokenless match, is returned
/// without a ref.
pub(super) fn publish_observed_matches(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    request: &LocatorResolveRequest,
    resolution: &mut LocatorResolution,
    observed: &ObservedTree,
) -> Result<(), AppError> {
    let mut refmap = RefMap::new();
    let observed_matches = std::mem::take(&mut resolution.matches);
    for mut matched in observed_matches {
        matched.data.ref_id = None;
        let token_backed = crate::refs::ref_identity::element_token(&matched.entry).is_some();
        if token_backed && !request.deadline.is_expired() {
            let Some(hydrated) =
                hydrate_one(adapter, query, request, (resolution, observed), matched)
            else {
                continue;
            };
            matched = hydrated;
            if matched.data.ref_id.is_some() {
                matched.data.ref_id = Some(refmap.try_allocate(matched.entry.clone())?);
            }
        }
        resolution.matches.push(matched);
    }
    resolution.refmap = (!refmap.is_empty()).then_some(refmap);
    Ok(())
}

fn hydrate_one(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    request: &LocatorResolveRequest,
    (resolution, observed): (&mut LocatorResolution, &ObservedTree),
    matched: LocatorMatch,
) -> Option<LocatorMatch> {
    let mut single = LocatorResolution {
        matches: vec![matched],
        refmap: None,
        stats: super::LocatorStats::default(),
        meta: resolution.meta.clone(),
    };
    let hydrated =
        super::hydrate::selected_matches(adapter, query, request, &mut single, observed).is_ok();
    resolution.stats.merge_observation(&single.stats);
    let mut matched = single.matches.pop()?;
    if !hydrated {
        matched.data.ref_id = None;
    }
    Some(matched)
}

#[cfg(test)]
#[path = "partial_tests.rs"]
mod tests;
