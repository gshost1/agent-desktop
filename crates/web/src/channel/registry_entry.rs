use serde::{Deserialize, Serialize};

/// One verified DevTools endpoint in `web/endpoints.json`: the browser
/// process launch started, the port it owns, the browser WebSocket URL its
/// `/json/version` advertised, and the download folder launch named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub pid: u32,
    pub process_instance: String,
    pub port: u16,
    pub websocket_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
}
