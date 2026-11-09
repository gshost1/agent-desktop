use serde::{Deserialize, Serialize};

/// Where a node's `name` came from when no accessible-name source supplied one.
///
/// A derived name locates (`--name`) and displays the element, but it is read
/// from a control value or from contained text, and neither is identity: a ref
/// entry and its capture proof leave it out, so it can never prove or
/// re-identify a ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameSource {
    /// The value of the element's sole text-field child, as a file list or
    /// outline row labelled by an inline, renamable text field reports it.
    ChildFieldValue,
    /// The text a web element contains, for an element that exposes no
    /// control and no accessible name but whose DOM node the page listens to
    /// for clicks, as a clickable `<div>` or `<span>` row reads.
    ContainedText,
}
