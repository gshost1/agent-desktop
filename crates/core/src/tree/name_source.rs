use serde::{Deserialize, Serialize};

/// Where a node's `name` came from when no accessible-name source supplied one.
///
/// A derived name locates (`--name`) and displays the element, but it is read
/// from a control value, and control values are never identity: a ref entry and
/// its capture proof leave it out, so it can never prove or re-identify a ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameSource {
    /// The value of the element's sole text-field child, as a file list or
    /// outline row labelled by an inline, renamable text field reports it.
    ChildFieldValue,
}
