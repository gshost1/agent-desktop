use std::path::PathBuf;

/// Where a DevTools endpoint candidate was found: the registry `launch --cdp`
/// writes, with the WebSocket URL it verified and the download folder launch
/// named, or a browser profile's `DevToolsActivePort` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateOrigin {
    Registry {
        websocket_url: String,
        download_dir: Option<PathBuf>,
    },
    ActivePort {
        user_data_dir: PathBuf,
    },
}
