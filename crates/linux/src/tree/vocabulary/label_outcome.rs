/// What reading a node's label and descriptive fallback produced.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum LabelOutcome {
    /// The node has no label relation, or its target has no text.
    Unlabelled,
    /// The label resolved and its text is readable.
    Text(String),
    /// An unlabelled control's sole-child wrapper supplies its description.
    WrapperDescription(String),
    /// The unlabelled control's wrapper could not be read.
    WrapperUnread,
    /// The relation or the target's name could not be read.
    Failed,
}
