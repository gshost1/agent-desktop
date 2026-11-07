use serde::{Deserialize, Serialize};

/// One verified DevTools endpoint in `web/endpoints.json`: the browser
/// process launch started, the port it owns, and the browser WebSocket URL
/// its `/json/version` advertised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub pid: u32,
    pub process_instance: String,
    pub port: u16,
    pub websocket_url: String,
}
