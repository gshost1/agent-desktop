use std::path::PathBuf;
use std::time::Duration;

use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::{AdapterError, Deadline, ProcessIdentity, StateFile};

use super::candidate_origin::CandidateOrigin;
use super::cdp_connector::CdpConnector;
use super::endpoint_candidate::EndpointCandidate;
use super::endpoint_mismatch::endpoint_mismatch;
use super::endpoint_registry::{read_entries, record_entry, registry_file};
use super::prompt_refusal::approval_needed;
use super::registry_entry::RegistryEntry;
use crate::{
    BrowserEndpoint, CdpConnection, EndpointSource, WebSocketUrl, connect, default_user_data_dirs,
    endpoint_from_port, read_active_port,
};

const VERSION_PROBE: Duration = Duration::from_millis(1_000);

/// The connector the binary uses. Candidates come from two places only: the
/// registry `launch --cdp` writes under the state root, and the
/// `DevToolsActivePort` file in each known browser's default profile.
/// `DevToolsActivePort` is only read, never rewritten or removed.
pub struct SystemConnector {
    registry: Option<StateFile>,
    user_data_dirs: Vec<PathBuf>,
}

impl SystemConnector {
    pub fn new() -> Self {
        Self {
            registry: registry_file().ok(),
            user_data_dirs: default_user_data_dirs()
                .into_iter()
                .map(|entry| entry.path)
                .collect(),
        }
    }

    /// A connector over an explicit registry and profile list.
    pub fn with_sources(registry: Option<StateFile>, user_data_dirs: Vec<PathBuf>) -> Self {
        Self {
            registry,
            user_data_dirs,
        }
    }
}

impl Default for SystemConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl CdpConnector for SystemConnector {
    fn candidates(&self, process: &ProcessIdentity) -> Vec<EndpointCandidate> {
        let pid = u32::from(process.pid);
        let mut candidates: Vec<EndpointCandidate> = self
            .registry
            .as_ref()
            .map(read_entries)
            .unwrap_or_default()
            .into_iter()
            .rev()
            .filter(|entry| entry.pid == pid && entry.process_instance == process.instance)
            .map(|entry| EndpointCandidate {
                port: entry.port,
                origin: CandidateOrigin::Registry {
                    websocket_url: entry.websocket_url,
                },
            })
            .collect();
        for dir in &self.user_data_dirs {
            if let Ok(active) = read_active_port(dir) {
                candidates.push(EndpointCandidate {
                    port: active.port,
                    origin: CandidateOrigin::ActivePort {
                        websocket_path: active.websocket_path,
                    },
                });
            }
        }
        candidates
    }

    fn open(
        &self,
        candidate: &EndpointCandidate,
        deadline: Deadline,
        may_prompt: bool,
    ) -> Result<CdpConnection, AdapterError> {
        let endpoint = match &candidate.origin {
            CandidateOrigin::Registry { websocket_url } => BrowserEndpoint {
                websocket_url: WebSocketUrl::parse(websocket_url)?,
                source: EndpointSource::JsonVersion,
                product: None,
            },
            CandidateOrigin::ActivePort { websocket_path } => {
                match endpoint_from_port(candidate.port, deadline.capped(VERSION_PROBE)) {
                    Ok(endpoint) => endpoint,
                    Err(_) if may_prompt => BrowserEndpoint {
                        websocket_url: WebSocketUrl::loopback(candidate.port, websocket_path)?,
                        source: EndpointSource::DevToolsActivePort,
                        product: None,
                    },
                    Err(_) => return Err(approval_needed(candidate.port)),
                }
            }
        };
        if endpoint.port() != candidate.port {
            return Err(endpoint_mismatch(candidate.port, endpoint.port()));
        }
        connect(&endpoint, deadline)
    }

    fn remember(
        &self,
        process: &ProcessIdentity,
        endpoint: &CdpEndpoint,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let Some(registry) = &self.registry else {
            return Ok(());
        };
        record_entry(
            registry,
            RegistryEntry {
                pid: u32::from(process.pid),
                process_instance: process.instance.clone(),
                port: endpoint.port,
                websocket_url: endpoint.websocket_url.clone(),
            },
            deadline,
        )
    }
}

#[cfg(test)]
#[path = "system_connector_tests.rs"]
mod tests;
