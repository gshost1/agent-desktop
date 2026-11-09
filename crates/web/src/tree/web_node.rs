use agent_desktop_core::AccessibilityNode;

use super::inline_run::InlineRun;
use crate::NodeOrigin;

/// A node under construction: the core node without its children, where it
/// came from, its children still in this form until frames are grafted, and
/// the text it adds to the block that holds it.
#[derive(Debug, Clone)]
pub(crate) struct WebNode {
    pub(crate) node: AccessibilityNode,
    pub(crate) origin: Option<NodeOrigin>,
    pub(crate) children: Vec<WebNode>,
    pub(crate) run: Option<InlineRun>,
}
