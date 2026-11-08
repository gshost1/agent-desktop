use serde_json::Value;

/// A JavaScript function to run with a DOM node bound as `this`, and the
/// JSON arguments to pass it.
pub(crate) struct ElementCall {
    pub(crate) function: &'static str,
    pub(crate) arguments: Vec<Value>,
}
