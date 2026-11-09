#[derive(Debug, Clone, Copy)]
pub(crate) struct ObservationCompleteness {
    pub(crate) subtree_complete: bool,
    pub(crate) predecessors_complete: bool,
    /// On a root: the observation left only list and table rows unread, on
    /// request. See [`super::ObservedTree::mark_rows_unread`].
    pub(crate) rows_unread: bool,
}

impl ObservationCompleteness {
    pub(crate) fn new(subtree_complete: bool) -> Self {
        Self {
            subtree_complete,
            predecessors_complete: true,
            rows_unread: false,
        }
    }
}
