use std::collections::HashSet;

use serde_json::Value;

/// The one computed style a DOM capture asks for: its value tells an element
/// laid out inside a line of text from one that starts its own box.
pub(crate) const DISPLAY_STYLE: &str = "display";

/// The backend node ids of the nodes a capture laid out inline: those whose
/// computed `display` is `inline`, or `contents`, which lays its children out
/// in its parent's box. Every other laid-out element, `inline-block` and
/// `inline-flex` included, holds its own box, so text never merges across
/// it. A node with no layout object is in neither set. The capture's
/// `layout.styles` holds, per layout object, one index into `strings` for
/// each requested style, and `layout.nodeIndex` points into the document's
/// nodes.
pub(crate) fn collect_inline(capture: &Value) -> HashSet<i64> {
    let empty = Vec::new();
    let strings = capture
        .get("strings")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let documents = capture
        .get("documents")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut inline = HashSet::new();
    for document in documents {
        let backends = document
            .pointer("/nodes/backendNodeId")
            .and_then(Value::as_array);
        let node_indexes = document
            .pointer("/layout/nodeIndex")
            .and_then(Value::as_array);
        let styles = document.pointer("/layout/styles").and_then(Value::as_array);
        let (Some(backends), Some(node_indexes), Some(styles)) = (backends, node_indexes, styles)
        else {
            continue;
        };
        for (node_index, style) in node_indexes.iter().zip(styles) {
            let display = style
                .as_array()
                .and_then(|values| values.first())
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| strings.get(index))
                .and_then(Value::as_str);
            if !display.is_some_and(is_inline_display) {
                continue;
            }
            let backend = node_index
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| backends.get(index))
                .and_then(Value::as_i64);
            inline.extend(backend);
        }
    }
    inline
}

fn is_inline_display(display: &str) -> bool {
    matches!(display.trim(), "inline" | "contents")
}

#[cfg(test)]
#[path = "dom_display_tests.rs"]
mod tests;
