use crate::{EndpointSource, WebSocketUrl};

/// A browser-level DevTools WebSocket endpoint on loopback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEndpoint {
    pub websocket_url: WebSocketUrl,
    pub source: EndpointSource,
    pub product: Option<String>,
}

impl BrowserEndpoint {
    pub fn port(&self) -> u16 {
        self.websocket_url.port()
    }
}
