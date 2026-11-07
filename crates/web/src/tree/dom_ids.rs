use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use crate::{CdpConnection, PageSession};

/// Reads the DOM `id` attribute of every element in the page, piercing
/// same-process frames and shadow roots, keyed by backend node id. One
/// `DOM.getDocument` call serves the whole snapshot.
pub(crate) fn read_dom_ids(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<HashMap<i64, String>, AdapterError> {
    let document = connection.call(
        "DOM.getDocument",
        json!({ "depth": -1, "pierce": true }),
        Some(&session.session_id),
        deadline,
    )?;
    Ok(collect_dom_ids(
        document.get("root").unwrap_or(&Value::Null),
    ))
}

pub(crate) fn collect_dom_ids(root: &Value) -> HashMap<i64, String> {
    let mut ids = HashMap::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if let (Some(backend), Some(id)) = (
            node.get("backendNodeId").and_then(Value::as_i64),
            id_attribute(node),
        ) {
            ids.insert(backend, id);
        }
        for key in ["children", "shadowRoots", "pseudoElements"] {
            if let Some(list) = node.get(key).and_then(Value::as_array) {
                pending.extend(list);
            }
        }
        for key in ["contentDocument", "templateContent"] {
            if let Some(child) = node.get(key) {
                pending.push(child);
            }
        }
    }
    ids
}

fn id_attribute(node: &Value) -> Option<String> {
    let attributes = node.get("attributes")?.as_array()?;
    attributes
        .chunks(2)
        .find(|pair| pair.first().and_then(Value::as_str) == Some("id"))
        .and_then(|pair| pair.get(1))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}
