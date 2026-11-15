//! The read a `wait --text` poll takes before it builds a snapshot.

use crate::{
    LocatorQuery,
    adapter::{PlatformAdapter, TreeOptions},
    locator::live_locator::{
        LocatorMaterialization, LocatorResolution, LocatorResolveRequest, LocatorSelection,
        ObservationRequest, evaluate_locator_tree,
    },
    snapshot::{surface_target, window_capture},
};

const MAX_RAW_DEPTH: u8 = 50;

/// Whether one locator read of the window the snapshot would capture shows
/// the text is absent from every node the snapshot would hold. The read asks
/// only for role, name, description and value, which an adapter that honors
/// the evidence plan reads far faster than a snapshot, and it is cut at the
/// snapshot's own depth. Any failure, any match and any incompleteness other
/// than that depth cut answers `false`, so the poll builds its snapshot as
/// before and a match is always judged from the snapshot.
pub(crate) fn rules_out_text(
    adapter: &dyn PlatformAdapter,
    text: &str,
    app: Option<&str>,
    opts: &TreeOptions,
    deadline: crate::Deadline,
) -> bool {
    let query = LocatorQuery {
        has_text: Some(text.to_string()),
        ..LocatorQuery::default()
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::Count,
        deadline,
        max_raw_depth: MAX_RAW_DEPTH,
        surface: None,
        materialization: LocatorMaterialization::None,
    };
    let Ok(owners) = surface_target::resolve_owners(adapter, app, None, opts.surface, deadline)
    else {
        return false;
    };
    window_capture::first_surface_owner(owners, |owner| {
        let root = owner.observation_root(opts.surface);
        let mut observation =
            ObservationRequest::locator_for_root(&query, &request, root, deadline);
        observation.max_logical_depth = ObservationRequest::snapshot_logical_depth(opts);
        let observation = observation.validate()?;
        let tree =
            crate::tree::renderer_accessibility::observe_tree(adapter, root, &observation, None)?;
        Ok(evaluate_locator_tree(tree, &query, &request)?)
    })
    .is_ok_and(|(_, resolution)| absent_within_depth(&resolution))
}

/// No match, and nothing left unread except what lies past the depth cut,
/// which the snapshot does not hold either.
fn absent_within_depth(resolution: &LocatorResolution) -> bool {
    if resolution.meta.total_matches > 0 {
        return false;
    }
    if resolution.meta.complete {
        return true;
    }
    let limits = &resolution.stats.traversal.limits;
    let health = &resolution.stats.reads.health;
    limits.depth_hits > 0
        && limits.node_hits == 0
        && limits.edge_hits == 0
        && limits.child_hits == 0
        && limits.text_hits == 0
        && limits.child_count_changes == 0
        && health.cannot_complete == 0
        && health.deadline_exhausted == 0
        && health.invalidated_elements == 0
}

#[cfg(test)]
#[path = "wait_text_probe_tests.rs"]
mod tests;
