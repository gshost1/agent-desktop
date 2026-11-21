use super::target_read::TargetReads;
use crate::PageSession;

/// One target a snapshot reads. An out-of-process iframe target also names
/// the parent target that holds its owner element, and the parent frame the
/// browser reported for it.
pub(crate) struct TargetJob<'a> {
    pub(crate) session: &'a PageSession,
    pub(crate) reads: TargetReads,
    pub(crate) parent: Option<&'a PageSession>,
    pub(crate) parent_frame_id: Option<&'a str>,
}
