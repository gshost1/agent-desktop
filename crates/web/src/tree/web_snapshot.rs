use agent_desktop_core::AccessibilityNode;

use crate::NodeOrigin;

/// A page's accessibility tree in core form. `origins[i]` belongs to the
/// node at depth-first pre-order index `i` of `tree` (the root is 0); it is
/// `None` for a node with no DOM backing. Every node that can receive a ref
/// also carries its element token in `identity.retained_object`, so the
/// side table is a convenience. `unreadable_frames` lists child frames whose
/// tree the page session could not read, such as out-of-process iframes.
#[derive(Debug, Clone)]
pub struct WebSnapshot {
    pub tree: AccessibilityNode,
    pub origins: Vec<Option<NodeOrigin>>,
    pub unreadable_frames: Vec<String>,
}

impl WebSnapshot {
    /// Every node in depth-first pre-order beside its origin.
    pub fn preorder(&self) -> Vec<(&AccessibilityNode, Option<&NodeOrigin>)> {
        let mut nodes = Vec::with_capacity(self.origins.len());
        let mut pending = vec![&self.tree];
        while let Some(node) = pending.pop() {
            let origin = self.origins.get(nodes.len()).and_then(Option::as_ref);
            nodes.push((node, origin));
            pending.extend(node.children.iter().rev());
        }
        nodes
    }
}
