use super::candidate_origin::CandidateOrigin;

/// A loopback port that may be a browser's DevTools endpoint. It is used only
/// after the platform adapter proves the browser process owns its listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointCandidate {
    pub port: u16,
    pub origin: CandidateOrigin,
}
