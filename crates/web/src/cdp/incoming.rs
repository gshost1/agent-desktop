use serde_json::Value;

use super::cdp_event::CdpEvent;

/// One parsed inbound frame: a reply to a request id, or an event.
pub(crate) enum Incoming {
    Reply { id: u64, body: Value },
    Event(CdpEvent),
    Unrecognized,
}

impl Incoming {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let mut frame: Value = serde_json::from_str(text).ok()?;
        if let Some(id) = frame.get("id").and_then(Value::as_u64) {
            return Some(Self::Reply {
                id,
                body: frame.take(),
            });
        }
        let Some(method) = frame.get("method").and_then(Value::as_str) else {
            return Some(Self::Unrecognized);
        };
        Some(Self::Event(CdpEvent {
            method: method.to_owned(),
            session_id: frame
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_owned),
            params: frame.get_mut("params").map_or(Value::Null, Value::take),
        }))
    }
}
