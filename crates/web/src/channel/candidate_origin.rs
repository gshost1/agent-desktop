use super::download::download_folder::DownloadFolder;

/// Where a DevTools endpoint candidate was found: the registry `launch --cdp`
/// writes, with the WebSocket URL it verified and the download folder launch
/// named, or a browser profile's `DevToolsActivePort` file, with the WebSocket
/// path read from it together with the port. The connection always goes to
/// the candidate's own port, the one whose listener ownership was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateOrigin {
    Registry {
        websocket_url: String,
        download_folder: Option<DownloadFolder>,
    },
    ActivePort {
        websocket_path: String,
    },
}
