/// Where a DevTools endpoint candidate was found: the registry `launch --cdp`
/// writes, with the WebSocket URL it verified, or a browser profile's
/// `DevToolsActivePort` file, with the WebSocket path read from it together
/// with the port. The connection always goes to the candidate's own port,
/// the one whose listener ownership was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateOrigin {
    Registry { websocket_url: String },
    ActivePort { websocket_path: String },
}
