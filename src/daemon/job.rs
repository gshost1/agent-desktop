//! One command the client said go for, on its way from a connection thread to the executor.

use std::sync::{Arc, Mutex};

use super::job_state::JobState;
use super::request::DaemonRequest;
use super::transport::DaemonStream;

/// The write half of a connection, shared by the executor's replies and the heartbeats.
pub(crate) type SharedWriter = Arc<Mutex<Box<dyn DaemonStream>>>;

/// A command the daemon took, with the connection its answer goes back on.
pub(crate) struct Job {
    pub(crate) writer: SharedWriter,
    pub(crate) request: DaemonRequest,
    pub(crate) state: Arc<JobState>,
}
