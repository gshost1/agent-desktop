use std::collections::{HashMap, HashSet};

use serde_json::json;

use super::{capture_params, collect_clickable, collect_dom_ids, dom_facts_from_capture};

#[test]
fn ids_come_from_the_flat_capture_of_every_document() {
    let capture = json!({
        "strings": ["id", "go", "class", "primary", "more", "  "],
        "documents": [
            { "nodes": {
                "backendNodeId": [1, 20, 30],
                "attributes": [[], [0, 1, 2, 3], [2, 1]],
            } },
            { "nodes": {
                "backendNodeId": [100, 101, 102],
                "attributes": [[], [0, 4], [0, 5]],
            } },
        ],
    });

    let ids = collect_dom_ids(&capture);

    assert_eq!(
        ids,
        HashMap::from([(20, "go".to_owned()), (101, "more".to_owned())])
    );
}

#[test]
fn a_dom_too_deep_for_a_nested_reply_is_read_from_one_flat_capture() {
    let depth = 400;
    let capture = json!({
        "strings": ["id", "leaf"],
        "documents": [{ "nodes": {
            "backendNodeId": (1..=depth).collect::<Vec<i64>>(),
            "attributes": (1..=depth)
                .map(|backend| if backend == depth { json!([0, 1]) } else { json!([]) })
                .collect::<Vec<_>>(),
        } }],
    });
    let facts = dom_facts_from_capture(&capture);

    assert_eq!(facts.ids.get(&depth).map(String::as_str), Some("leaf"));
    assert_eq!(capture_params(), json!({ "computedStyles": ["display"] }));
}

#[test]
fn clickable_nodes_come_from_each_documents_is_clickable_indexes() {
    let capture = json!({
        "strings": [],
        "documents": [
            { "nodes": {
                "backendNodeId": [1, 20, 30],
                "isClickable": { "index": [1, 7] },
            } },
            { "nodes": { "backendNodeId": [100, 101] } },
            { "nodes": {
                "backendNodeId": [200, 201],
                "isClickable": { "index": [0] },
            } },
        ],
    });

    assert_eq!(collect_clickable(&capture), HashSet::from([20, 200]));
}

#[test]
fn a_documents_html_and_body_are_never_clickable_targets() {
    let capture = json!({
        "strings": ["#document", "HTML", "BODY", "DIV", "body"],
        "documents": [
            { "nodes": {
                "backendNodeId": [1, 2, 3, 4],
                "nodeName": [0, 1, 2, 3],
                "isClickable": { "index": [1, 2, 3] },
            } },
            { "nodes": {
                "backendNodeId": [10, 11],
                "nodeName": [4, 3],
                "isClickable": { "index": [0, 1] },
            } },
        ],
    });

    assert_eq!(collect_clickable(&capture), HashSet::from([4, 11]));
}
