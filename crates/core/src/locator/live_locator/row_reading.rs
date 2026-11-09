/// Whether an observation reads the rows of list and table containers.
///
/// A `--first` resolution asks first for a read that leaves those rows unread,
/// so a control beside a long list is found without reading the list. A
/// platform that reads rows last honors it; any other platform reads the whole
/// tree, which answers the same query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowReading {
    All,
    LeaveUnread,
}
