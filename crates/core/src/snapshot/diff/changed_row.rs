use serde_json::{Map, Value, json};

use crate::{AccessibilityNode, AppError};

/// A node both snapshots hold under the same identity whose visible fields
/// differ: the node as it reads now, which fields changed, and the node as it
/// read before.
#[derive(Debug, Clone)]
pub(crate) struct ChangedRow {
    pub(crate) now: AccessibilityNode,
    pub(crate) fields: Vec<&'static str>,
    pub(crate) before: AccessibilityNode,
}

impl ChangedRow {
    /// The row as it prints: the current node's fields, `changed` naming the
    /// fields, and `was` holding each changed field's earlier value, `null`
    /// when it had none. `states` and `focus` both report the earlier states.
    pub(crate) fn to_json(&self) -> Result<Value, AppError> {
        let mut row = serde_json::to_value(&self.now)?;
        let mut was = Map::new();
        for field in &self.fields {
            let (key, value) = match *field {
                "name" => ("name", json!(self.before.identity.name)),
                "value" => ("value", json!(self.before.identity.value)),
                "description" => ("description", json!(self.before.identity.description)),
                "placeholder" => (
                    "placeholder",
                    json!(self.before.presentation.descriptors.placeholder),
                ),
                "hint" => ("hint", json!(self.before.presentation.hint)),
                _ => ("states", json!(self.before.presentation.states)),
            };
            was.insert(key.into(), value);
        }
        if let Some(body) = row.as_object_mut() {
            body.insert("changed".into(), json!(self.fields));
            body.insert("was".into(), Value::Object(was));
        }
        Ok(row)
    }
}
