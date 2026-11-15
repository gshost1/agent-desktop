//! The OS seam: how a client reaches its daemon and how a daemon takes connections. Everything
//! above this trait (framing, admission, execution, fallback) is the same on every platform; a
//! platform without an implementation runs every command in the calling process.

use std::io::{Read, Write};
use std::time::Duration;

use super::endpoint::DaemonEndpoint;

/// One open connection between a client and its daemon.
pub(crate) trait DaemonStream: Read + Write + Send {
    /// A second handle to the same connection, for a thread that only reads.
    fn try_clone_stream(&self) -> std::io::Result<Box<dyn DaemonStream>>;
    /// Bounds every later read; `None` waits without limit.
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()>;
}

/// A daemon's listening endpoint.
pub(crate) trait DaemonListener: Send {
    /// The next connection, or `None` for a peer that is not this user running this executable,
    /// which is closed unanswered.
    fn accept_trusted(&self) -> std::io::Result<Option<Box<dyn DaemonStream>>>;
}

/// Connects clients and binds daemons for one platform.
pub(crate) trait DaemonTransport: Sync {
    fn connect(&self, endpoint: &DaemonEndpoint) -> std::io::Result<Box<dyn DaemonStream>>;
    /// Binds `endpoint` for its lock holder, replacing what a dead daemon left there.
    fn listen(&self, endpoint: &DaemonEndpoint) -> std::io::Result<Box<dyn DaemonListener>>;
    /// Whether `error` from `connect` means no daemon serves the endpoint, so one may be started.
    fn is_absent(&self, error: &std::io::Error) -> bool;
}
