use agent_desktop_core::Deadline;
use serde_json::Value;

/// One request of a pipelined batch: the method, its params, the flattened
/// session it addresses and the deadline its reply must arrive by.
#[derive(Debug, Clone)]
pub(crate) struct CdpRequest {
    pub(crate) method: &'static str,
    pub(crate) params: Value,
    pub(crate) session_id: Option<String>,
    pub(crate) deadline: Deadline,
}
