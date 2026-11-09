use std::collections::{HashMap, HashSet};

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::dom_display::{DISPLAY_STYLE, collect_inline};
use super::dom_facts::DomFacts;
use crate::{CdpConnection, PageSession};

/// Reads the DOM `id` attribute of every element in the target's local
/// frames and shadow trees, the nodes that respond to mouse clicks, and the
/// nodes laid out inline, keyed by backend node id. One `DOMSnapshot.captureSnapshot` serves the whole
/// target: its reply is flat arrays however deep the DOM nests, and unlike
/// `DOM.getDocument` it does not switch on the DOM domain's mutation events.
pub(crate) fn read_dom_facts(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<DomFacts, AdapterError> {
    let capture = connection.call(
        "DOMSnapshot.captureSnapshot",
        json!({ "computedStyles": [DISPLAY_STYLE] }),
        Some(&session.session_id),
        deadline,
    )?;
    Ok(DomFacts {
        ids: collect_dom_ids(&capture),
        clickable: collect_clickable(&capture),
        inline: collect_inline(&capture),
    })
}

/// The backend node ids the capture's `isClickable` list names: nodes with a
/// `click`, `mousedown` or `mouseup` listener of their own, links, and
/// editable content. The list holds indexes into each document's nodes. A
/// document's `<html>` and `<body>` are left out: a listener there handles
/// clicks for the whole page and makes neither a click target.
pub(crate) fn collect_clickable(capture: &Value) -> HashSet<i64> {
    let empty = Vec::new();
    let strings = capture
        .get("strings")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let is_page_root = |name: Option<&Value>| {
        name.and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| strings.get(index))
            .and_then(Value::as_str)
            .is_some_and(|name| {
                name.eq_ignore_ascii_case("html") || name.eq_ignore_ascii_case("body")
            })
    };
    let documents = capture
        .get("documents")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut clickable = HashSet::new();
    for nodes in documents
        .iter()
        .filter_map(|document| document.get("nodes"))
    {
        let backends = nodes.get("backendNodeId").and_then(Value::as_array);
        let indexes = nodes
            .pointer("/isClickable/index")
            .and_then(Value::as_array);
        let (Some(backends), Some(indexes)) = (backends, indexes) else {
            continue;
        };
        let names = nodes.get("nodeName").and_then(Value::as_array);
        clickable.extend(
            indexes
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|index| usize::try_from(index).ok())
                .filter(|index| !is_page_root(names.and_then(|names| names.get(*index))))
                .filter_map(|index| backends.get(index))
                .filter_map(Value::as_i64),
        );
    }
    clickable
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
