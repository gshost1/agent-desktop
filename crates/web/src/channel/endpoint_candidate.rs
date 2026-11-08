use super::candidate_origin::CandidateOrigin;
use super::download::download_folder::DownloadFolder;

/// A loopback port that may be a browser's DevTools endpoint. It is used only
/// after the platform adapter proves the browser process owns its listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointCandidate {
    pub port: u16,
    pub origin: CandidateOrigin,
}

impl EndpointCandidate {
    /// The folder downloads go to when this endpoint is used: only a browser
    /// that `launch --cdp --download-dir` started has one. A browser found
    /// through its profile keeps its own download folder.
    pub fn download_folder(&self) -> Option<DownloadFolder> {
        match &self.origin {
            CandidateOrigin::Registry {
                download_folder, ..
            } => download_folder.clone(),
            CandidateOrigin::ActivePort { .. } => None,
        }
    }
}
