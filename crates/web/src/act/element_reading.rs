use serde::{Deserialize, Serialize};

/// A live read of a DOM node: its form value, its rendered text (capped at
/// 10,000 characters) and its checked state, each absent when the node has
/// none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementReading {
    pub value: Option<String>,
    pub text: Option<String>,
    pub checked: Option<bool>,
}
