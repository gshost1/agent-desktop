use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use crate::act::act_fixture::{deadline, resolved, session, token};
use crate::act::fill::fill;
use crate::act::fill_outcome::FillOutcome;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{AssignedInput, CdpConnection};

type Sent = std::sync::Arc<std::sync::Mutex<Vec<Value>>>;

fn prepared(reply: Value) -> ScriptedTransport {
    resolved(ScriptedTransport::new()).expect(
        "Runtime.callFunctionOn",
        json!({ "result": { "value": reply } }),
    )
}

fn assigned(input_type: &str, reply: Value) -> ScriptedTransport {
    prepared(json!({ "valueType": input_type }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": reply } }),
        )
        .expect("Runtime.releaseObject", json!({}))
}

fn set(transport: ScriptedTransport, text: &str) -> (Result<FillOutcome, AdapterError>, Sent) {
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    let outcome = fill(&mut connection, &session(), &token(7), text, deadline());
    (outcome, sent)
}

fn assignments(sent: &Sent) -> Vec<Value> {
    sent.lock()
        .unwrap()
        .iter()
        .filter(|request| request["method"] == "Runtime.callFunctionOn")
        .skip(1)
        .map(|request| request["params"]["arguments"][0]["value"].clone())
        .collect()
}

#[test]
fn each_input_type_is_assigned_its_value_and_verified_by_the_read_back() {
    for (input_type, value) in [
        ("date", "2026-11-08"),
        ("time", "19:30"),
        ("datetime-local", "2026-11-08T19:30"),
        ("month", "2026-11"),
        ("week", "2026-W45"),
        ("color", "#ff8800"),
        ("range", "70"),
    ] {
        let reply = json!({ "status": "assigned", "observed": value });
        let (outcome, sent) = set(assigned(input_type, reply), value);
        let outcome = outcome.unwrap();

        assert_eq!(outcome.delivery(), DeliverySemantics::delivered_verified());
        assert_eq!(outcome.observed.as_deref(), Some(value));
        assert_eq!(
            outcome.assigned,
            Some(AssignedInput {
                input_type: input_type.into(),
                unchanged: false,
            })
        );
        assert_eq!(assignments(&sent), [json!(value)], "{input_type}");
        assert!(
            sent.lock()
                .unwrap()
                .iter()
                .all(|request| request["method"] != "Input.insertText"),
            "{input_type} took typed text"
        );
    }
}

#[test]
fn an_input_that_already_holds_the_value_is_satisfied_without_delivery() {
    let reply = json!({ "status": "unchanged", "observed": "19:30" });
    let outcome = set(assigned("time", reply), "19:30").0.unwrap();

    assert_eq!(outcome.delivery(), DeliverySemantics::not_delivered());
    assert!(outcome.assigned.unwrap().unchanged);
}

#[test]
fn a_read_back_that_cannot_be_read_leaves_the_assignment_unverified() {
    let reply = json!({ "status": "assigned", "observed": null });
    let outcome = set(assigned("date", reply), "2026-11-08").0.unwrap();

    assert_eq!(
        outcome.delivery(),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(outcome.observed, None);
}

#[test]
fn a_read_back_that_differs_is_a_failure_that_names_the_observed_value() {
    let reply = json!({ "status": "assigned", "observed": "100" });
    let error = set(assigned("range", reply), "500").0.unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "cdp_value_adjusted");
    assert_eq!(details["observed"], "100");
    assert_eq!(details["value"], "500");
}

#[test]
fn a_value_the_input_rejects_fails_and_is_not_delivered() {
    let reply = json!({ "status": "rejected", "observed": "2026-01-01" });
    let error = set(assigned("date", reply), "2026-02-30").0.unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.details.unwrap()["kind"], "cdp_value_rejected");
    assert!(error.suggestion.unwrap().contains("YYYY-MM-DD"));
}

#[test]
fn malformed_text_is_refused_with_the_format_before_the_page_is_touched() {
    for (input_type, text, format) in [
        ("date", "11/08/2026", "YYYY-MM-DD"),
        ("time", "07", "HH:MM"),
        ("datetime-local", "tomorrow", "YYYY-MM-DDTHH:MM"),
        ("month", "Nov", "YYYY-MM"),
        ("week", "45", "YYYY-Www"),
        ("color", "red", "#rrggbb"),
        ("range", "loud", "a number"),
    ] {
        let transport =
            prepared(json!({ "valueType": input_type })).expect("Runtime.releaseObject", json!({}));
        let (outcome, sent) = set(transport, text);
        let error = outcome.unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidArgs, "{input_type}");
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert!(error.suggestion.unwrap().contains(format), "{input_type}");
        assert!(assignments(&sent).is_empty(), "{input_type} was assigned");
    }
}

#[test]
fn another_form_of_a_value_is_refused_with_the_exact_value_to_pass() {
    let transport =
        prepared(json!({ "valueType": "time" })).expect("Runtime.releaseObject", json!({}));
    let (outcome, sent) = set(transport, "7:30 PM");
    let error = outcome.unwrap_err();

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(error.suggestion.unwrap().starts_with("Pass '19:30'."));
    assert_eq!(error.details.unwrap()["value"], "19:30");
    assert!(assignments(&sent).is_empty());
}

#[test]
fn one_part_of_a_time_input_is_refused_and_points_at_the_input() {
    let transport = prepared(json!({ "editable": false, "partOf": "time" }))
        .expect("Runtime.releaseObject", json!({}));
    let (outcome, sent) = set(transport, "07");
    let error = outcome.unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.suggestion.unwrap().contains("time input's own ref"));
    assert_eq!(error.details.unwrap()["kind"], "cdp_input_part");
    assert!(
        sent.lock()
            .unwrap()
            .iter()
            .all(|request| request["method"] != "Input.insertText")
    );
}
