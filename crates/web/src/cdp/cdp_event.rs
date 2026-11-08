use serde_json::Value;

/// An unsolicited CDP message: a protocol event, with the flattened session
/// it arrived on when it came from an attached target.
#[derive(Debug, Clone, PartialEq)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}
