use std::path::Path;

use agent_desktop_core::AdapterError;

use super::active_port::read_active_port;
use crate::{BrowserEndpoint, EndpointSource, WebSocketUrl};

/// Builds the browser endpoint advertised by a profile's `DevToolsActivePort`
/// file. This is the only way to reach a browser whose remote debugging was
/// enabled from chrome://inspect: that server speaks WebSocket only and has
/// no `/json` HTTP routes. Nothing is contacted here; [`crate::connect`]
/// does that and waits for the browser's approval prompt.
pub fn endpoint_from_user_data_dir(user_data_dir: &Path) -> Result<BrowserEndpoint, AdapterError> {
    let active = read_active_port(user_data_dir)?;
    Ok(BrowserEndpoint {
        websocket_url: WebSocketUrl::loopback(active.port, &active.websocket_path)?,
        source: EndpointSource::DevToolsActivePort,
        product: None,
    })
}
