use std::collections::{HashMap, HashSet};

/// What one `DOMSnapshot.captureSnapshot` of a target says about its DOM
/// nodes, keyed by backend node id: each element's `id` attribute, the
/// nodes the renderer reports as responding to mouse clicks, and the nodes
/// laid out inline, inside the line of text around them.
#[derive(Debug, Clone, Default)]
pub(crate) struct DomFacts {
    pub(crate) ids: HashMap<i64, String>,
    pub(crate) clickable: HashSet<i64>,
    pub(crate) inline: HashSet<i64>,
}
