use std::collections::HashMap;

/// What a frame's tree build needs besides the AX nodes: the page target
/// and frame the nodes belong to, and the DOM `id` attribute of each
/// backend node.
pub(crate) struct FrameContext<'a> {
    pub(crate) target_id: &'a str,
    pub(crate) frame_id: &'a str,
    pub(crate) dom_ids: &'a HashMap<i64, String>,
}
