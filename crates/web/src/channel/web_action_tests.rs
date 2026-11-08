use agent_desktop_core::{
    Action, ActionOps, ActionRequest, DeliverySemantics, Direction, ErrorCode, KeyCombo, Modifier,
    NativeHandle, ProcessIdentity,
};
use serde_json::{Value, json};

use crate::ElementToken;
use crate::channel::test_support::{
    PORT, ScriptedConnector, StubPlatform, TOKEN, adapter, attach, element_read, lease, process,
    proven,
};
use crate::channel::web_handle::WebHandle;
use crate::transport::scripted_transport::ScriptedTransport;

fn web_handle() -> NativeHandle {
    NativeHandle::new(WebHandle {
        process: process(),
        token: ElementToken::parse(TOKEN).unwrap(),
    })
}

fn fill_script(value_after: Value) -> ScriptedTransport {
    proven(attach(ScriptedTransport::new(), "T"))
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "s" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": { "editable": true } } }),
        )
        .expect("Input.insertText", json!({}))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": value_after } }),
        )
        .expect("Runtime.releaseObject", json!({}))
}

fn run(
    transport: ScriptedTransport,
    action: Action,
) -> Result<agent_desktop_core::ActionResult, agent_desktop_core::AdapterError> {
    let web = adapter(
        StubPlatform::owning(PORT),
        ScriptedConnector::with(transport),
    );
    let result = web.execute_action(&web_handle(), ActionRequest::headless(action), &lease());
    assert!(
        web.inner().recorded().is_empty(),
        "a web handle reached the platform"
    );
    result
}

#[test]
fn set_value_fills_the_field_and_is_verified_by_its_read_back() {
    let result = run(
        fill_script(json!("hello")),
        Action::SetValue("hello".into()),
    )
    .unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(result.details.unwrap()["channel"], "cdp");
}

#[test]
fn set_value_the_read_back_disagrees_with_stays_unverified_and_reports_it() {
    let result = run(fill_script(json!("hel")), Action::SetValue("hello".into())).unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
    assert_eq!(result.details.unwrap()["observed"], "hel");
}

#[test]
fn click_dispatches_page_input_and_stays_unverified() {
    let transport = proven(attach(ScriptedTransport::new(), "T"))
        .expect("DOM.scrollIntoViewIfNeeded", json!({}))
        .expect(
            "Page.getLayoutMetrics",
            crate::act::act_fixture::metrics(0.0),
        )
        .expect(
            "DOM.getContentQuads",
            json!({ "quads": [[0, 0, 10, 0, 10, 10, 0, 10]] }),
        )
        .expect("DOM.getNodeForLocation", json!({ "backendNodeId": 7 }))
        .expect("Input.dispatchMouseEvent", json!({}))
        .expect("Input.dispatchMouseEvent", json!({}));
    let sent = transport.sent_log();
    let result = run(transport, Action::Click).unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
    let sent = sent.lock().unwrap();
    let page_input_only = [
        "Target.attachToTarget",
        "Runtime.runIfWaitingForDebugger",
        "Page.getFrameTree",
        "DOM.scrollIntoViewIfNeeded",
        "Page.getLayoutMetrics",
        "DOM.getContentQuads",
        "DOM.getNodeForLocation",
        "Input.dispatchMouseEvent",
    ];
    assert!(
        sent.iter().all(
            |request| page_input_only.contains(&request["method"].as_str().unwrap_or_default())
        )
    );
}

#[test]
fn checking_a_checked_box_delivers_nothing() {
    let transport = element_read(
        attach(ScriptedTransport::new(), "T"),
        json!("on"),
        json!(true),
    );
    let result = run(transport, Action::Check).unwrap();
    assert_eq!(result.disposition(), DeliverySemantics::not_delivered());
}

#[test]
fn checking_an_element_without_checked_state_is_not_supported() {
    let transport = element_read(
        attach(ScriptedTransport::new(), "T"),
        json!("x"),
        Value::Null,
    );
    let error = run(transport, Action::Check).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}

#[test]
fn scrolling_reports_whether_the_page_moved() {
    let transport = proven(attach(ScriptedTransport::new(), "T"))
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "o" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": true } }),
        )
        .expect("Runtime.releaseObject", json!({}));
    let result = run(transport, Action::Scroll(Direction::Down, 3)).unwrap();
    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
}

#[test]
fn a_key_with_modifiers_is_not_supported_before_anything_is_sent() {
    let combo = KeyCombo {
        key: "a".into(),
        modifiers: vec![Modifier::Meta],
    };
    let error = run(
        attach(ScriptedTransport::new(), "T"),
        Action::PressKey(combo),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}

#[test]
fn actions_without_a_page_mapping_are_not_supported() {
    for action in [Action::RightClick, Action::Hover, Action::Expand] {
        let error = run(attach(ScriptedTransport::new(), "T"), action).unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert!(error.suggestion.is_some());
    }
}

#[test]
fn a_request_for_another_process_is_refused_before_connecting() {
    let web = adapter(StubPlatform::owning(PORT), ScriptedConnector::default());
    let request = ActionRequest::headless(Action::Click)
        .with_expected_process(ProcessIdentity::new(99u32, "other"));
    let error = web
        .execute_action(&web_handle(), request, &lease())
        .unwrap_err();
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_native_handle_never_reaches_the_devtools_connection() {
    let connector = ScriptedConnector::default();
    let opens = connector.opens.clone();
    let web = adapter(StubPlatform::owning(PORT), connector);
    web.execute_action(
        &NativeHandle::new("native"),
        ActionRequest::headless(Action::Click),
        &lease(),
    )
    .unwrap();
    assert_eq!(web.inner().recorded(), vec!["execute_action"]);
    assert!(opens.lock().unwrap().is_empty());
}
