use agent_desktop_core::AccessibilityNode;

use super::web_node::WebNode;
use crate::NodeOrigin;

/// Turns a built node into a core tree and records each node's origin at
/// its depth-first pre-order index, the root being index 0.
pub(crate) fn flatten(root: WebNode, origins: &mut Vec<Option<NodeOrigin>>) -> AccessibilityNode {
    let WebNode {
        mut node,
        origin,
        children,
        ..
    } = root;
    origins.push(origin);
    node.children = children
        .into_iter()
        .map(|child| flatten(child, origins))
        .collect();
    node
}
