//! One admitted command on its way from a connection thread to the executor.

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use super::admission::Admitted;
use super::job_state::JobState;
use super::request::DaemonRequest;

/// A command the daemon took, with the connection its answer goes back on.
pub(crate) struct Job {
    pub(crate) stream: UnixStream,
    pub(crate) request: DaemonRequest,
    pub(crate) admitted: Admitted,
    pub(crate) state: Arc<JobState>,
}
