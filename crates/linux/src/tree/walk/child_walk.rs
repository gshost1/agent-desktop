#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChildWalk {
    pub(super) max_edges: usize,
    pub(super) max_per_node: usize,
    pub(super) prune_closed_menus: bool,
}
