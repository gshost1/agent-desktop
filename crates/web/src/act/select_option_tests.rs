use agent_desktop_core::ErrorCode;
use serde_json::{Value, json};

use super::select_option;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, resolved, session, token};
use crate::transport::scripted_transport::ScriptedTransport;

fn answering(outcome: Value) -> CdpConnection {
    let transport = resolved(ScriptedTransport::new())
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": outcome } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    CdpConnection::new(transport.boxed())
}

#[test]
fn a_matching_option_returns_the_new_value() {
    let mut connection = answering(json!({ "value": "green", "label": "Green" }));
    let value = select_option(&mut connection, &session(), &token(4), "Green", deadline()).unwrap();
    assert_eq!(value, "green");
}

#[test]
fn a_missing_option_lists_the_available_ones() {
    let mut connection = answering(json!({ "error": "no_option", "options": ["Red", "Green"] }));
    let error =
        select_option(&mut connection, &session(), &token(4), "Blue", deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.as_ref().unwrap()["options"],
        json!(["Red", "Green"])
    );
}

#[test]
fn a_non_select_is_not_supported() {
    let mut connection = answering(json!({ "error": "not_select" }));
    let error =
        select_option(&mut connection, &session(), &token(4), "Green", deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}
