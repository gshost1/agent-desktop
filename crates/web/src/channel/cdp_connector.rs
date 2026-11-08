use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::{AdapterError, Deadline, ProcessIdentity};

use super::endpoint_candidate::EndpointCandidate;
use crate::CdpConnection;

/// Finds and opens DevTools endpoints. The web channel asks it for
/// candidates, keeps only those whose listener the browser process owns, and
/// opens the first; tests substitute a scripted connector.
pub trait CdpConnector: Send + Sync {
    /// Endpoints that may belong to `process`, best first. Nothing is
    /// contacted.
    fn candidates(&self, process: &ProcessIdentity) -> Vec<EndpointCandidate>;

    /// Opens a browser-level connection. `may_prompt` allows an endpoint
    /// whose browser asks its user to approve each connection; without it
    /// such an endpoint is refused so a read-only listing never shows a
    /// prompt.
    fn open(
        &self,
        candidate: &EndpointCandidate,
        deadline: Deadline,
        may_prompt: bool,
    ) -> Result<CdpConnection, AdapterError>;

    /// Records an endpoint `launch --cdp` verified the process owns.
    fn remember(
        &self,
        process: &ProcessIdentity,
        endpoint: &CdpEndpoint,
        deadline: Deadline,
    ) -> Result<(), AdapterError>;
}
