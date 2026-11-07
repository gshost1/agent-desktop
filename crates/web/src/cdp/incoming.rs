use serde_json::Value;

use super::cdp_event::CdpEvent;
use super::frame_envelope::FrameEnvelope;

/// One parsed inbound frame: a reply to a request id, an event, or a reply
/// whose body could not be read because it nests too deeply.
pub(crate) enum Incoming {
    Reply { id: u64, body: Value },
    Event(CdpEvent),
    UnreadableReply { id: u64 },
    Unrecognized,
}

impl Incoming {
    /// `None` only for text that is not JSON at all. A well-formed frame too
    /// deep to parse is an unreadable reply when it carries an id, and is
    /// otherwise an event this client cannot use and skips.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let mut frame: Value = match serde_json::from_str(text) {
            Ok(frame) => frame,
            Err(_) => {
                let envelope: FrameEnvelope = serde_json::from_str(text).ok()?;
                return Some(match envelope.id {
                    Some(id) => Self::UnreadableReply { id },
                    None => Self::Unrecognized,
                });
            }
        };
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
