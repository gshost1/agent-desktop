use serde::Deserialize;

/// A live read of a DOM node: its form value and its checked state, each
/// absent when the node has none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ElementReading {
    pub value: Option<String>,
    pub checked: Option<bool>,
}
