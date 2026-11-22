use agent_desktop_core::{ObservationRequest, SnapshotSurface};

use super::child_walk::ChildWalk;

const DEFAULT_MAX_RAW_DEPTH: usize = 50;
const DEFAULT_MAX_NODES: usize = 10_000;
const DEFAULT_MAX_EDGES: usize = 100_000;
const DEFAULT_MAX_CHILDREN: usize = 4_096;
const DEFAULT_MAX_FIELD_BYTES: usize = 64 * 1024;

/// How far a walk may go before it marks the rest truncated, and whether its
/// root already sits below a web document. Raw depth counts every node on the
/// path; logical depth skips wrapper nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WalkLimits {
    pub(crate) max_raw_depth: usize,
    pub(crate) max_logical_depth: usize,
    pub(crate) max_nodes: usize,
    pub(crate) children: ChildWalk,
    pub(crate) bulk_children: usize,
    pub(crate) max_field_bytes: usize,
    pub(crate) root_in_web: bool,
}

impl Default for WalkLimits {
    fn default() -> Self {
        Self {
            max_raw_depth: DEFAULT_MAX_RAW_DEPTH,
            max_logical_depth: DEFAULT_MAX_RAW_DEPTH,
            max_nodes: DEFAULT_MAX_NODES,
            children: ChildWalk {
                max_edges: DEFAULT_MAX_EDGES,
                max_per_node: DEFAULT_MAX_CHILDREN,
                prune_closed_menus: false,
            },
            bulk_children: DEFAULT_MAX_CHILDREN,
            max_field_bytes: DEFAULT_MAX_FIELD_BYTES,
            root_in_web: false,
        }
    }
}

impl WalkLimits {
    /// The limits an observation request asks for: its depths and its budget.
    pub(crate) fn for_request(request: &ObservationRequest) -> Self {
        Self {
            max_raw_depth: usize::from(request.max_raw_depth),
            max_logical_depth: usize::from(request.max_logical_depth),
            max_nodes: request.budget.max_nodes,
            children: ChildWalk {
                max_edges: request.budget.max_edges,
                max_per_node: request.budget.max_children_per_node,
                prune_closed_menus: request.surface == SnapshotSurface::Window,
            },
            bulk_children: request.budget.max_children_per_node,
            max_field_bytes: request.budget.max_field_bytes,
            root_in_web: false,
        }
    }
}
