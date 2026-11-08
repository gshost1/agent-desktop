use crate::PageSession;

/// An out-of-process iframe target attached beneath another target: its
/// flattened session, and the frame of the parent target that holds it when
/// the browser names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildTarget {
    pub(crate) session: PageSession,
    pub(crate) parent_frame_id: Option<String>,
}
