use super::dom_facts::DomFacts;

/// What a frame's tree build needs besides the AX nodes: the target and
/// frame the nodes belong to, the loader of the frame's document, and what
/// the target's DOM snapshot says about each backend node.
pub(crate) struct FrameContext<'a> {
    pub(crate) target_id: &'a str,
    pub(crate) frame_id: &'a str,
    pub(crate) loader_id: &'a str,
    pub(crate) dom: &'a DomFacts,
}
