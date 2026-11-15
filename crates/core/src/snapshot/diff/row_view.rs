use serde::{Serialize, Serializer, ser::SerializeSeq};

use crate::{AccessibilityNode, NodeDescriptor, NodeIdentity, snapshot::diff::diff_rows};

/// The reported rows of a tree, serialized straight from the tree in the
/// depth-first order refs are allocated in, so saving a baseline clones no
/// node. Each row reads back as an `AccessibilityNode` without bounds,
/// actions or children.
pub(crate) struct RowSeq<'a>(pub(crate) &'a AccessibilityNode);

impl Serialize for RowSeq<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        let mut pending = vec![self.0];
        while let Some(node) = pending.pop() {
            if diff_rows::is_reported(node) {
                seq.serialize_element(&RowView::of(node))?;
            }
            pending.extend(node.children.iter().rev());
        }
        seq.end()
    }
}

#[derive(Serialize)]
struct RowView<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_id: Option<&'a str>,
    role: &'a str,
    #[serde(flatten)]
    identity: &'a NodeIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    states: &'a [String],
    #[serde(flatten)]
    descriptors: &'a NodeDescriptor,
}

impl<'a> RowView<'a> {
    fn of(node: &'a AccessibilityNode) -> Self {
        Self {
            ref_id: node.ref_id.as_deref(),
            role: &node.role,
            identity: &node.identity,
            hint: node.presentation.hint.as_deref(),
            states: &node.presentation.states,
            descriptors: &node.presentation.descriptors,
        }
    }
}
