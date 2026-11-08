/// One frame of a target's `Page.getFrameTree`: its id, its parent frame
/// (absent only for a page's main frame) and the loader of the document it
/// shows now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameEntry {
    pub(crate) frame_id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) loader_id: String,
}
