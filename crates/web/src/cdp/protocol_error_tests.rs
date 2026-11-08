use agent_desktop_core::ErrorCode;
use serde_json::json;

use super::{is_node_gone, is_protocol_error, is_target_gone, protocol_error};

#[test]
fn a_lost_session_or_closed_target_is_target_gone_not_a_stale_node() {
    for (code, message) in [
        (-32001, "Session with given id not found."),
        (-32000, "No target with given id found"),
        (-32000, "Target closed"),
    ] {
        let error = protocol_error(
            "DOM.resolveNode",
            &json!({ "code": code, "message": message }),
        );
        assert_eq!(error.code, ErrorCode::WindowNotFound, "{message}");
        assert!(is_target_gone(&error), "{message}");
        assert!(!is_node_gone(&error), "{message}");
        assert!(!is_protocol_error(&error), "{message}");
    }
}

#[test]
fn only_a_missing_node_message_is_node_gone() {
    let gone = protocol_error(
        "DOM.resolveNode",
        &json!({ "code": -32000, "message": "No node with given id found" }),
    );
    let other = protocol_error(
        "DOM.resolveNode",
        &json!({ "code": -32000, "message": "Object couldn't be returned by value" }),
    );
    let missing_method = protocol_error(
        "DOM.resolveNode",
        &json!({ "code": -32601, "message": "'DOM.resolveNode' wasn't found" }),
    );

    assert!(is_node_gone(&gone));
    assert!(!is_node_gone(&other));
    assert!(!is_node_gone(&missing_method));
    assert_eq!(missing_method.code, ErrorCode::ActionNotSupported);
}
