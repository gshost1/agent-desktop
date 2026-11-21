//! Unix-domain sockets: `<key>.sock` bound 0600 inside the private daemon directory, with the
//! peer's credentials checked on every accepted connection.

use std::io::ErrorKind;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

use super::endpoint::DaemonEndpoint;
use super::peer::is_trusted_peer;
use super::transport::{DaemonListener, DaemonStream, DaemonTransport};
use super::unix_listener::UnixDaemonListener;

/// The transport on Linux and macOS.
pub(crate) struct UnixTransport;

impl DaemonStream for UnixStream {
    fn try_clone_stream(&self) -> std::io::Result<Box<dyn DaemonStream>> {
        Ok(Box::new(self.try_clone()?))
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        UnixStream::set_read_timeout(self, timeout)
    }

    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        UnixStream::set_write_timeout(self, timeout)
    }
}

impl DaemonTransport for UnixTransport {
    /// Connects and then checks the daemon the way it checks its clients, so a socket another
    /// user planted never receives this call's arguments.
    fn connect(&self, endpoint: &DaemonEndpoint) -> std::io::Result<Box<dyn DaemonStream>> {
        let stream = UnixStream::connect(endpoint.socket())?;
        if !is_trusted_peer(&stream) {
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "the daemon is not this user running this executable",
            ));
        }
        Ok(Box::new(stream))
    }

    fn listen(&self, endpoint: &DaemonEndpoint) -> std::io::Result<Box<dyn DaemonListener>> {
        Ok(Box::new(UnixDaemonListener::new(bind(&endpoint.socket())?)))
    }

    fn is_absent(&self, error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            ErrorKind::NotFound | ErrorKind::ConnectionRefused
        )
    }
}

/// Binds `path` after removing a socket a dead daemon left there. Only the lock holder gets
/// here, so the socket it removes cannot belong to a live daemon.
fn bind(path: &Path) -> std::io::Result<UnixListener> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.file_type().is_socket() {
            return Err(std::io::Error::other("not a socket"));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

#[cfg(test)]
#[path = "unix_transport_tests.rs"]
mod tests;
