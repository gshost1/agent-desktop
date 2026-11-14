//! What every thread of a running daemon shares: who it serves and whether it is busy.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::build_identity::BuildIdentity;
use super::endpoint::DaemonEndpoint;
use super::env_subset::EnvSubset;
use super::request::{DaemonRequest, PROTOCOL_VERSION};

const CONNECTION_LIMIT: usize = 64;

/// The identity a daemon serves and its one execution slot.
pub(crate) struct ServerState {
    pub(crate) endpoint: DaemonEndpoint,
    build: BuildIdentity,
    env: EnvSubset,
    busy: AtomicBool,
    connections: AtomicUsize,
}

impl ServerState {
    pub(crate) fn new(endpoint: DaemonEndpoint, build: BuildIdentity, env: EnvSubset) -> Self {
        Self {
            endpoint,
            build,
            env,
            busy: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
        }
    }

    /// Why this daemon cannot serve `request`, or `None` when its protocol, build and relevant
    /// environment all match the daemon's own.
    pub(crate) fn mismatch(&self, request: &DaemonRequest) -> Option<&'static str> {
        if request.protocol != PROTOCOL_VERSION {
            return Some("protocol");
        }
        if request.build != self.build.as_str() {
            return Some("build");
        }
        (request.env != self.env).then_some("environment")
    }

    /// Takes the execution slot; `false` while another command holds it.
    pub(crate) fn claim(&self) -> bool {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn release(&self) {
        self.busy.store(false, Ordering::Release);
    }

    /// Counts a new connection; `false` when too many are open.
    pub(crate) fn open_connection(&self) -> bool {
        if self.connections.fetch_add(1, Ordering::AcqRel) < CONNECTION_LIMIT {
            return true;
        }
        self.close_connection();
        false
    }

    pub(crate) fn close_connection(&self) {
        self.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
#[path = "server_state_tests.rs"]
mod tests;
