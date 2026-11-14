use crate::{
    AccessibilityNode, ElementIdentifier, refs::ref_alloc::identity_name,
    refs::ref_identity::stable_name, tree::accname::non_blank,
};

/// The identity a diff pairs nodes by: role plus the stable fields a ref's
/// capture proof records, that is native id, stable name and description.
/// Values, states and bounds are never part of it, so a node whose value
/// changed is still the same node, and a node whose stable name changed is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct IdentityKey<'a> {
    pub(crate) role: &'a str,
    pub(crate) native_id: Option<&'a ElementIdentifier>,
    pub(crate) name: Option<&'a str>,
    pub(crate) description: Option<&'a str>,
}

impl<'a> IdentityKey<'a> {
    pub(crate) fn of(node: &'a AccessibilityNode) -> Self {
        let value = node
            .identity
            .value
            .as_deref()
            .filter(|text| !text.is_empty());
        Self {
            role: &node.role,
            native_id: node
                .identity
                .native_id
                .as_ref()
                .filter(|identifier| !identifier.value.trim().is_empty()),
            name: stable_name(&node.role, identity_name(node), value),
            description: non_blank(node.identity.description.as_deref()),
        }
    }
}

#[cfg(test)]
#[path = "diff_key_tests.rs"]
mod tests;
