//! The read a `wait --text` poll takes before it builds a snapshot.

use crate::{
    LocatorQuery,
    adapter::{PlatformAdapter, TreeOptions},
    locator::live_locator::{
        LocatorMaterialization, LocatorResolution, LocatorResolveRequest, LocatorSelection,
        ObservationRequest, ObservedTree, evaluate_locator_tree,
    },
    snapshot::{surface_target, window_capture},
};

const MAX_RAW_DEPTH: u8 = 50;

/// What one text read told the poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextProbe {
    /// The text is in no node the snapshot could hold, so this poll needs no
    /// snapshot.
    pub(crate) ruled_out: bool,
    /// The adapter read only the fields the probe asked for. One that reads
    /// every field anyway makes the probe cost a snapshot's walk, so the wait
    /// stops probing.
    pub(crate) read_less: bool,
}

const UNDECIDED: TextProbe = TextProbe {
    ruled_out: false,
    read_less: true,
};

/// Reads the window the snapshot would capture for role, name, description
/// and value only, as deep as a locator walk reads, which is at least as deep
/// as any snapshot. Any failure, any match and any incompleteness other than
/// the raw depth cut a snapshot shares leaves the text possible, so the poll
/// builds its snapshot as before and a match is always judged from it.
pub(crate) fn probe_text(
    adapter: &dyn PlatformAdapter,
    text: &str,
    app: Option<&str>,
    opts: &TreeOptions,
    deadline: crate::Deadline,
) -> TextProbe {
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
        return UNDECIDED;
    };
    let read = window_capture::first_surface_owner(owners, |owner| {
        let root = owner.observation_root(opts.surface);
        let observation =
            ObservationRequest::locator_for_root(&query, &request, root, deadline).validate()?;
        let tree =
            crate::tree::renderer_accessibility::observe_tree(adapter, root, &observation, None)?;
        let read_less = read_only_what_was_asked(&tree);
        Ok((evaluate_locator_tree(tree, &query, &request)?, read_less))
    });
    match read {
        Ok((_, (resolution, read_less))) => TextProbe {
            ruled_out: absent_from_the_read(&resolution),
            read_less,
        },
        Err(_) => UNDECIDED,
    }
}

/// The probe asks for no states, so an adapter that honors the request
/// leaves every node's states unread.
fn read_only_what_was_asked(tree: &ObservedTree) -> bool {
    tree.nodes
        .iter()
        .all(|node| node.evidence.states.is_unknown())
}

/// No match, and nothing left unread but what lies past the raw depth cut,
/// which no snapshot reads either: no budget, child, text or label limit, no
/// failed or invalidated read and no unrecorded cause.
fn absent_from_the_read(resolution: &LocatorResolution) -> bool {
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
        && limits.child_label_hits == 0
        && limits.text_hits == 0
        && limits.child_count_changes == 0
        && health.cannot_complete == 0
        && health.native_read_failures == 0
        && health.deadline_exhausted == 0
        && health.invalidated_elements == 0
}

#[cfg(test)]
#[path = "wait_text_probe_tests.rs"]
mod tests;
