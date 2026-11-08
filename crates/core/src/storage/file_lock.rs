use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::{AdapterError, Deadline, ErrorCode};

/// A private state-file lock held until this guard is dropped.
///
/// Releases by closing rather than unlocking: a duplicated descriptor shares
/// the lock, so unlocking would release it for the other holder too.
pub struct FileLock {
    _file: File,
    #[cfg(unix)]
    contention_count: u64,
}

impl FileLock {
    pub fn acquire(path: &Path, deadline: Deadline, purpose: &str) -> Result<Self, AdapterError> {
        let file =
            crate::storage::private_file::open_private_lock(path, true).map_err(state_io_error)?;
        lock_file(file, deadline, purpose, path)
    }

    pub(crate) fn is_held(path: &Path) -> bool {
        let Ok(file) = crate::storage::private_file::open_private_lock(path, false) else {
            return false;
        };
        match file.try_lock() {
            Ok(()) => {
                let _ = file.unlock();
                false
            }
            Err(std::fs::TryLockError::WouldBlock) => true,
            Err(std::fs::TryLockError::Error(_)) => true,
        }
    }

    #[cfg(unix)]
    pub(crate) fn contention_count(&self) -> u64 {
        self.contention_count
    }

    #[cfg(unix)]
    pub(crate) fn duplicate_inheritable(&self) -> Result<std::os::fd::OwnedFd, AdapterError> {
        use std::os::fd::{AsRawFd, FromRawFd};

        let duplicated = unsafe { libc::fcntl(self._file.as_raw_fd(), libc::F_DUPFD, 3) };
        if duplicated < 0 {
            return Err(state_io_error(std::io::Error::last_os_error()));
        }
        Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(duplicated) })
    }

    #[cfg(unix)]
    pub(crate) fn adopt_inherited(
        raw_fd: std::os::fd::RawFd,
        canonical_path: &Path,
        deadline: Deadline,
        purpose: &str,
    ) -> Result<Self, AdapterError> {
        use std::os::fd::{FromRawFd, OwnedFd};
        use std::os::unix::fs::MetadataExt;

        let duplicated = unsafe { libc::fcntl(raw_fd, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicated < 0 {
            return Err(state_io_error(std::io::Error::last_os_error()));
        }
        let original_flags = unsafe { libc::fcntl(raw_fd, libc::F_GETFD) };
        if original_flags < 0
            || unsafe { libc::fcntl(raw_fd, libc::F_SETFD, original_flags | libc::FD_CLOEXEC) } != 0
        {
            unsafe { libc::close(duplicated) };
            return Err(state_io_error(std::io::Error::last_os_error()));
        }
        let inherited = File::from(unsafe { OwnedFd::from_raw_fd(duplicated) });
        let inherited_meta = crate::storage::private_file::validate_private_regular(&inherited)
            .map_err(state_io_error)?;
        let canonical = crate::storage::private_file::open_private_lock(canonical_path, false)
            .map_err(state_io_error)?;
        let canonical_meta = canonical.metadata().map_err(state_io_error)?;
        if inherited_meta.dev() != canonical_meta.dev()
            || inherited_meta.ino() != canonical_meta.ino()
        {
            return Err(AdapterError::new(
                ErrorCode::PolicyDenied,
                "Inherited interaction lease does not identify the canonical lock file",
            ));
        }
        lock_file(inherited, deadline, purpose, canonical_path)
    }
}

/// Drops a contended lock without unlocking: shared file descriptions need drop,
/// not unlock. On the adopt path, shared descriptions exist and must be left alone.
fn lock_file(
    file: File,
    deadline: Deadline,
    purpose: &str,
    path: &Path,
) -> Result<FileLock, AdapterError> {
    let mut contention_count = 0_u64;
    loop {
        if deadline.is_expired() {
            return Err(lock_timeout(deadline, purpose, path, contention_count));
        }
        match file.try_lock() {
            Ok(()) => {
                if deadline.is_expired() {
                    return Err(lock_timeout(deadline, purpose, path, contention_count));
                }
                return Ok(FileLock {
                    _file: file,
                    #[cfg(unix)]
                    contention_count,
                });
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                contention_count = contention_count.saturating_add(1);
                let remaining = deadline.remaining();
                if remaining.is_zero() {
                    return Err(lock_timeout(deadline, purpose, path, contention_count));
                }
                crate::wait::deadline::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(AdapterError::new(
                    ErrorCode::Internal,
                    format!("Failed to acquire {purpose}: {error}"),
                ));
            }
        }
    }
}

fn lock_timeout(
    deadline: Deadline,
    purpose: &str,
    path: &Path,
    contention_count: u64,
) -> AdapterError {
    deadline.timeout_error().with_details(serde_json::json!({
        "kind": "lock_timeout",
        "purpose": purpose,
        "path": path.file_name().map(|name| name.to_string_lossy().into_owned()),
        "contention_count": contention_count,
    }))
}

/// A permission refusal on agent-desktop's own state files keeps
/// `PERM_DENIED` and names the fix. The caller can repair a state directory
/// that other users can read, so `INTERNAL`, which offers no action, is the
/// wrong answer for it.
pub(crate) fn state_io_error(error: std::io::Error) -> AdapterError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return AdapterError::new(ErrorCode::PermDenied, error.to_string())
            .with_suggestion(
                "agent-desktop keeps its state private. Make the state directory \
                 (AGENT_DESKTOP_HOME, default ~/.agent-desktop) and the folders inside it owned \
                 by you and closed to other users: chmod 700 <dir>",
            )
            .with_details(serde_json::json!({
                "kind": "state_directory_permission",
                "retryable": false,
            }));
    }
    AdapterError::new(ErrorCode::Internal, error.to_string())
}

#[cfg(test)]
#[path = "file_lock_tests.rs"]
mod tests;
