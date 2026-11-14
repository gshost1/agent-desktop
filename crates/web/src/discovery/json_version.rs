use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

use super::http_get::http_get_loopback;
use crate::{BrowserEndpoint, EndpointSource, WebSocketUrl};

/// Discovers the browser endpoint behind an explicit remote-debugging port by
/// reading `/json/version`. Core's validator decides what counts: the
/// advertised `webSocketDebuggerUrl` must be a `ws://` loopback URL on that
/// same port.
pub fn endpoint_from_port(port: u16, deadline: Deadline) -> Result<BrowserEndpoint, AdapterError> {
    let body = http_get_loopback(port, "/json/version", deadline)?;
    parse_version_body(port, &body)
}

pub(crate) fn parse_version_body(port: u16, body: &str) -> Result<BrowserEndpoint, AdapterError> {
    let endpoint = agent_desktop_core::parse_devtools_version_body(port, body)
        .ok_or_else(|| not_devtools(port))?;
    Ok(BrowserEndpoint {
        websocket_url: WebSocketUrl::parse(&endpoint.websocket_url)?,
        source: EndpointSource::JsonVersion,
    })
}

fn not_devtools(port: u16) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "The server on port {port} did not answer /json/version as a loopback DevTools endpoint"
        ),
    )
    .with_details(serde_json::json!({ "kind": "cdp_endpoint_invalid", "port": port }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "json_version_tests.rs"]
mod tests;
