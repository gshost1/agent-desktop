use super::input_value_kind::InputValueKind;

/// One whole-value assignment: the input's kind, the text the caller asked
/// for, and the DOM node it names, for error reports.
pub(crate) struct ValueTarget<'a> {
    pub(crate) kind: InputValueKind,
    pub(crate) text: &'a str,
    pub(crate) backend_node_id: i64,
}
