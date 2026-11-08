use agent_desktop_core::AccessibilityNode;

use crate::NodeOrigin;

/// A node under construction: the core node without its children, where it
/// came from, and its children still in this form until frames are grafted.
#[derive(Debug, Clone)]
pub(crate) struct WebNode {
    pub(crate) node: AccessibilityNode,
    pub(crate) origin: Option<NodeOrigin>,
    pub(crate) children: Vec<WebNode>,
}
