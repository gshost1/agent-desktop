use std::collections::HashSet;

use serde_json::json;

use super::collect_inline;

#[test]
fn only_inline_and_contents_layout_objects_are_inline() {
    let capture = json!({
        "strings": ["block", "inline", "inline-block", "contents", "list-item", "inline-flex"],
        "documents": [
            { "nodes": { "backendNodeId": [1, 2, 3, 4, 5, 6, 7] },
              "layout": {
                  "nodeIndex": [0, 1, 2, 3, 4, 5],
                  "styles": [[0], [1], [2], [3], [4], [5]],
              } },
            { "nodes": { "backendNodeId": [100, 101] },
              "layout": { "nodeIndex": [1], "styles": [[1]] } },
        ],
    });

    assert_eq!(collect_inline(&capture), HashSet::from([2, 4, 101]));
}

#[test]
fn a_capture_without_layout_styles_marks_nothing_inline() {
    let capture = json!({
        "strings": ["inline"],
        "documents": [
            { "nodes": { "backendNodeId": [1, 2] }, "layout": { "nodeIndex": [0, 1] } },
            { "nodes": { "backendNodeId": [3] } },
        ],
    });

    assert!(collect_inline(&capture).is_empty());
}
