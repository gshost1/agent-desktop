use agent_desktop_core::{Deadline, ErrorCode};
use serde_json::json;

use super::{press_key, supported_key_names};
use crate::act::key_definition::find_key;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{CdpConnection, PageSession};

fn session() -> PageSession {
    PageSession {
        target_id: "T".into(),
        session_id: "S".into(),
    }
}

#[test]
fn keys_resolve_by_name_and_alias() {
    assert_eq!(find_key("enter").unwrap().key_code, 13);
    assert_eq!(find_key("Return").unwrap().key, "Enter");
    assert_eq!(find_key("esc").unwrap().key, "Escape");
    assert_eq!(find_key("down").unwrap().key, "ArrowDown");
    assert_eq!(find_key("space").unwrap().code, "Space");
    assert!(find_key("F13").is_none());
    assert!(supported_key_names().contains(&"Backspace"));
    assert!(supported_key_names().contains(&"Space"));
}

#[test]
fn a_text_key_sends_down_char_and_up() {
    let transport = ScriptedTransport::new()
        .expect("Input.dispatchKeyEvent", json!({}))
        .expect("Input.dispatchKeyEvent", json!({}))
        .expect("Input.dispatchKeyEvent", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    press_key(
        &mut connection,
        &session(),
        "Enter",
        Deadline::after(1_000).unwrap(),
    )
    .unwrap();

    let sent = sent.lock().unwrap();
    let types: Vec<_> = sent
        .iter()
        .map(|request| request["params"]["type"].clone())
        .collect();
    assert_eq!(types, [json!("keyDown"), json!("char"), json!("keyUp")]);
    assert_eq!(sent[1]["params"]["text"], "\r");
    assert_eq!(sent[0]["params"]["windowsVirtualKeyCode"], 13);
}

#[test]
fn a_control_key_sends_raw_down_and_up() {
    let transport = ScriptedTransport::new()
        .expect("Input.dispatchKeyEvent", json!({}))
        .expect("Input.dispatchKeyEvent", json!({}));
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    press_key(
        &mut connection,
        &session(),
        "Tab",
        Deadline::after(1_000).unwrap(),
    )
    .unwrap();

    let sent = sent.lock().unwrap();
    assert_eq!(sent[0]["params"]["type"], "rawKeyDown");
    assert_eq!(sent[1]["params"]["type"], "keyUp");
}

#[test]
fn an_unknown_key_is_refused_before_anything_is_sent() {
    let mut connection = CdpConnection::new(ScriptedTransport::new().boxed());
    let error = press_key(
        &mut connection,
        &session(),
        "Hyper",
        Deadline::after(1_000).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}
