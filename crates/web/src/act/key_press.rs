use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::key_definition::{KEYS, find_key};
use crate::{CdpConnection, PageSession};

/// Presses and releases one named key in the page's focused element. Keys
/// that type text also send the `char` event that inserts it.
pub fn press_key(
    connection: &mut CdpConnection,
    session: &PageSession,
    key_name: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let definition = find_key(key_name).ok_or_else(|| unknown_key(key_name))?;
    let base = json!({
        "key": definition.key,
        "code": definition.code,
        "windowsVirtualKeyCode": definition.key_code,
        "nativeVirtualKeyCode": definition.key_code,
    });
    let mut events = Vec::with_capacity(3);
    let mut down = base.clone();
    down["type"] = json!(if definition.text.is_some() {
        "keyDown"
    } else {
        "rawKeyDown"
    });
    events.push(down);
    if let Some(text) = definition.text {
        let mut typed = base.clone();
        typed["type"] = json!("char");
        typed["text"] = json!(text);
        typed["unmodifiedText"] = json!(text);
        events.push(typed);
    }
    let mut up = base;
    up["type"] = json!("keyUp");
    events.push(up);
    for event in events {
        connection.call(
            "Input.dispatchKeyEvent",
            event,
            Some(&session.session_id),
            deadline,
        )?;
    }
    Ok(())
}

/// The key names [`press_key`] accepts.
pub fn supported_key_names() -> Vec<&'static str> {
    KEYS.iter()
        .map(|definition| {
            if definition.key == " " {
                "Space"
            } else {
                definition.key
            }
        })
        .collect()
}

fn unknown_key(name: &str) -> AdapterError {
    AdapterError::new(ErrorCode::InvalidArgs, format!("Unsupported key '{name}'"))
        .with_suggestion(format!("Use one of: {}", supported_key_names().join(", ")))
        .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "key_press_tests.rs"]
mod tests;
