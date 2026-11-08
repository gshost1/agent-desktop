use agent_desktop_core::{DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use super::fill;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, resolved, session, token};
use crate::transport::scripted_transport::ScriptedTransport;

fn answer(transport: ScriptedTransport, value: Value) -> ScriptedTransport {
    transport.expect(
        "Runtime.callFunctionOn",
        json!({ "result": { "value": value } }),
    )
}

fn filled(read_back: &str) -> ScriptedTransport {
    let transport = answer(
        resolved(ScriptedTransport::new()),
        json!({ "editable": true }),
    )
    .expect("Input.insertText", json!({}));
    answer(transport, json!(read_back)).expect("Runtime.releaseObject", json!({}))
}

#[test]
fn a_fill_that_reads_back_the_text_is_verified_in_six_round_trips() {
    let transport = filled("Ada");
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let outcome = fill(&mut connection, &session(), &token(7), "Ada", deadline()).unwrap();

    assert!(outcome.verified);
    assert_eq!(outcome.observed.as_deref(), Some("Ada"));
    assert_eq!(outcome.delivery(), DeliverySemantics::delivered_verified());
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 6);
    assert_eq!(sent[3]["params"]["text"], "Ada");
}

#[test]
fn a_page_that_rewrites_the_text_leaves_the_fill_unverified() {
    let mut connection = CdpConnection::new(filled("ADA").boxed());

    let outcome = fill(&mut connection, &session(), &token(7), "Ada", deadline()).unwrap();

    assert!(!outcome.verified);
    assert_eq!(outcome.observed.as_deref(), Some("ADA"));
    assert_eq!(
        outcome.delivery(),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn a_target_that_takes_no_text_is_refused_before_typing() {
    let transport = answer(
        resolved(ScriptedTransport::new()),
        json!({ "editable": false, "reason": "input type checkbox" }),
    )
    .expect("Runtime.releaseObject", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let error = fill(&mut connection, &session(), &token(7), "Ada", deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("input type checkbox")
    );
    let sent = sent.lock().unwrap();
    assert!(
        sent.iter()
            .all(|request| request["method"] != "Input.insertText")
    );
}
