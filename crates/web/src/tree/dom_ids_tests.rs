use std::collections::{HashMap, HashSet};

use agent_desktop_core::Deadline;
use serde_json::json;

use super::{collect_clickable, collect_dom_ids, read_dom_facts};
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

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
    let transport = ScriptedTransport::new().expect("DOMSnapshot.captureSnapshot", capture);
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let session = PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    };

    let facts = read_dom_facts(&mut connection, &session, Deadline::after(1_000).unwrap()).unwrap();

    assert_eq!(facts.ids.get(&depth).map(String::as_str), Some("leaf"));
    let sent = sent.lock().unwrap();
    assert_eq!(sent[0]["params"], json!({ "computedStyles": [] }));
    assert!(connection.take_events().is_empty());
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
