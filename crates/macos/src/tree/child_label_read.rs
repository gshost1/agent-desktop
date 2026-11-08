use crate::tree::AXElement;

/// What one bounded pass over an element's children found: the static-text
/// label, whether the pass read everything it examined, whether the element had
/// more children than the pass examines, and the text fields it met on the way.
pub(crate) struct ChildLabelRead {
    pub(crate) label: Option<String>,
    pub(crate) complete: bool,
    pub(crate) capped: bool,
    pub(crate) text_fields: Vec<AXElement>,
    pub(crate) secure_fields: usize,
}
