use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::{Value, json};

use super::select_option;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

fn answering(outcome: Value) -> CdpConnection {
    let transport = ScriptedTransport::new()
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "O" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": outcome } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    CdpConnection::new(transport.boxed())
}

fn deadline() -> Deadline {
    Deadline::after(1_000).unwrap()
}

#[test]
fn a_matching_option_returns_the_new_value() {
    let mut connection = answering(json!({ "value": "green", "label": "Green" }));
    let value = select_option(&mut connection, &session(), 4, "Green", deadline()).unwrap();
    assert_eq!(value, "green");
}

#[test]
fn a_missing_option_lists_the_available_ones() {
    let mut connection = answering(json!({ "error": "no_option", "options": ["Red", "Green"] }));
    let error = select_option(&mut connection, &session(), 4, "Blue", deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.as_ref().unwrap()["options"],
        json!(["Red", "Green"])
    );
}

#[test]
fn a_non_select_is_not_supported() {
    let mut connection = answering(json!({ "error": "not_select" }));
    let error = select_option(&mut connection, &session(), 4, "Green", deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}
