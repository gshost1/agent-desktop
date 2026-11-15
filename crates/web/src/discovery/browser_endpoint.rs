use crate::{EndpointSource, WebSocketUrl};

/// A browser-level DevTools WebSocket endpoint on loopback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEndpoint {
    pub websocket_url: WebSocketUrl,
    pub(crate) source: EndpointSource,
}

impl BrowserEndpoint {
    pub fn port(&self) -> u16 {
        self.websocket_url.port()
    }
}
