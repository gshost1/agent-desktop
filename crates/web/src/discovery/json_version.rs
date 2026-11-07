use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::Value;

use super::http_get::http_get_loopback;
use crate::{BrowserEndpoint, EndpointSource, WebSocketUrl};

/// Discovers the browser endpoint behind an explicit remote-debugging port by
/// reading `/json/version`. The advertised `webSocketDebuggerUrl` must be a
/// loopback URL on that same port.
pub fn endpoint_from_port(port: u16, deadline: Deadline) -> Result<BrowserEndpoint, AdapterError> {
    let body = http_get_loopback(port, "/json/version", deadline)?;
    parse_version_body(port, &body)
}

pub(crate) fn parse_version_body(port: u16, body: &str) -> Result<BrowserEndpoint, AdapterError> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|error| not_devtools(port).with_platform_detail(error.to_string()))?;
    let advertised = parsed
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| not_devtools(port))?;
    let websocket_url = WebSocketUrl::parse(advertised)?;
    if websocket_url.port() != port {
        return Err(not_devtools(port).with_platform_detail(format!(
            "webSocketDebuggerUrl names port {} instead of {port}",
            websocket_url.port()
        )));
    }
    Ok(BrowserEndpoint {
        websocket_url,
        source: EndpointSource::JsonVersion,
        product: parsed
            .get("Browser")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn not_devtools(port: u16) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The server on port {port} did not answer /json/version as a DevTools endpoint"),
    )
    .with_details(serde_json::json!({ "kind": "cdp_endpoint_invalid", "port": port }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "json_version_tests.rs"]
mod tests;
