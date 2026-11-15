//! A bound Unix-domain socket that serves only peers running as this user and this executable.

use std::os::unix::net::UnixListener;

use super::peer::is_trusted_peer;
use super::transport::{DaemonListener, DaemonStream};

/// The daemon's listening socket.
pub(crate) struct UnixDaemonListener(UnixListener);

impl UnixDaemonListener {
    pub(crate) fn new(listener: UnixListener) -> Self {
        Self(listener)
    }
}

impl DaemonListener for UnixDaemonListener {
    fn accept_trusted(&self) -> std::io::Result<Option<Box<dyn DaemonStream>>> {
        let (stream, _) = self.0.accept()?;
        if !is_trusted_peer(&stream) {
            return Ok(None);
        }
        Ok(Some(Box::new(stream)))
    }
}
