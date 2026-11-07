use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline, ProcessIdentity};

use crate::cdp::protocol_error::is_protocol_error;
use crate::{CdpConnection, PageSession, attach_to_target};

/// One open browser-level DevTools connection for one browser process, and
/// the page sessions attached through it. The adapter keeps it for the rest
/// of the invocation so each read does not reconnect.
pub struct BrowserLink {
    pub process: ProcessIdentity,
    pub connection: CdpConnection,
    sessions: HashMap<String, PageSession>,
}

impl BrowserLink {
    pub fn new(process: ProcessIdentity, connection: CdpConnection) -> Self {
        Self {
            process,
            connection,
            sessions: HashMap::new(),
        }
    }

    /// The flattened session for `target_id`, attaching once. A target the
    /// browser no longer has is `STALE_REF`.
    pub fn session(
        &mut self,
        target_id: &str,
        deadline: Deadline,
    ) -> Result<PageSession, AdapterError> {
        if let Some(session) = self.sessions.get(target_id) {
            return Ok(session.clone());
        }
        let session =
            attach_to_target(&mut self.connection, target_id, deadline).map_err(|error| {
                if is_protocol_error(&error) {
                    let detail = error.platform_detail.clone().unwrap_or_default();
                    AdapterError::stale_ref_because(format!(
                        "Page target {target_id} is no longer open"
                    ))
                    .with_platform_detail(detail)
                    .with_details(serde_json::json!({
                        "kind": "cdp_target_gone",
                        "target_id": target_id,
                    }))
                } else {
                    error
                }
            })?;
        self.sessions.insert(target_id.to_owned(), session.clone());
        Ok(session)
    }
}
