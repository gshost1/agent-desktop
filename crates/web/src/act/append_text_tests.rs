use serde_json::json;

use super::append_text;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, proven, resolved, session, token};
use crate::transport::scripted_transport::ScriptedTransport;

fn caret_moved() -> ScriptedTransport {
    resolved(proven(ScriptedTransport::new()).expect("DOM.focus", json!({})))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "type": "string", "value": "control" } }),
        )
        .expect("Runtime.releaseObject", json!({}))
}

#[test]
fn text_is_inserted_after_the_caret_moves_to_the_end() {
    let transport = caret_moved().expect("Input.insertText", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    append_text(&mut connection, &session(), &token(5), " world", deadline()).unwrap();
    let sent = sent.lock().unwrap();
    let insert = sent.last().unwrap();
    assert_eq!(insert["params"]["text"], " world");
    assert!(
        sent[4]["params"]["functionDeclaration"]
            .as_str()
            .unwrap()
            .contains("setSelectionRange(end, end)")
    );
}

#[test]
fn empty_text_only_places_the_caret() {
    let transport = caret_moved();
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());
    append_text(&mut connection, &session(), &token(5), "", deadline()).unwrap();
    assert!(
        sent.lock()
            .unwrap()
            .iter()
            .all(|message| message["method"] != "Input.insertText")
    );
}
