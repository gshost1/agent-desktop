/// What a node under construction contributes to the reading-order text of
/// the block that holds it. Static text and line breaks carry their text;
/// a block edge is a marker, removed before a node is emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InlineRun {
    /// A static-text run, with the spaces at its ends as the page laid it
    /// out. A line break is a run of `"\n"`.
    Text(String),
    /// The edge of a block-level element whose node was spliced out of the
    /// tree. Text never merges across it.
    BlockEdge,
}
