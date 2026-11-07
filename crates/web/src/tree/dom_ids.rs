use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use crate::{CdpConnection, PageSession};

/// Reads the DOM `id` attribute of every element in the target's local
/// frames and shadow trees, keyed by backend node id. One
/// `DOMSnapshot.captureSnapshot` serves the whole target: its reply is flat
/// arrays however deep the DOM nests, and unlike `DOM.getDocument` it does
/// not switch on the DOM domain's mutation events.
pub(crate) fn read_dom_ids(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<HashMap<i64, String>, AdapterError> {
    let capture = connection.call(
        "DOMSnapshot.captureSnapshot",
        json!({ "computedStyles": [] }),
        Some(&session.session_id),
        deadline,
    )?;
    Ok(collect_dom_ids(&capture))
}

/// Pairs each node's `backendNodeId` with its `id` attribute. Attribute
/// names and values are indexes into the capture's shared `strings` table.
pub(crate) fn collect_dom_ids(capture: &Value) -> HashMap<i64, String> {
    let empty = Vec::new();
    let strings = capture
        .get("strings")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let string = |index: &Value| {
        index
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| strings.get(index))
            .and_then(Value::as_str)
    };
    let documents = capture
        .get("documents")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut ids = HashMap::new();
    for nodes in documents
        .iter()
        .filter_map(|document| document.get("nodes"))
    {
        let backends = nodes.get("backendNodeId").and_then(Value::as_array);
        let attributes = nodes.get("attributes").and_then(Value::as_array);
        let (Some(backends), Some(attributes)) = (backends, attributes) else {
            continue;
        };
        for (backend, pairs) in backends.iter().zip(attributes) {
            let id = pairs.as_array().and_then(|pairs| {
                pairs
                    .chunks(2)
                    .find(|pair| pair.first().and_then(string) == Some("id"))
                    .and_then(|pair| pair.get(1))
                    .and_then(string)
            });
            let id = id.map(str::trim).filter(|id| !id.is_empty());
            if let (Some(backend), Some(id)) = (backend.as_i64(), id) {
                ids.insert(backend, id.to_owned());
            }
        }
    }
    ids
}

#[cfg(test)]
#[path = "dom_ids_tests.rs"]
mod tests;
