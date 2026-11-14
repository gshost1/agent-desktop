//! The daemon directory: created 0700, and trusted only while it is a real directory owned by
//! this user that no other user can enter.

use std::io::{Error, ErrorKind};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::Path;

/// Creates `dir` and its missing parents as 0700, then checks that it is private.
pub(crate) fn ensure_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    check_private_dir(dir)
}

/// Succeeds when `dir` is a directory (not a symlink) owned by the effective user with no group
/// or other permission bits.
pub(crate) fn check_private_dir(dir: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(dir)?;
    let owner = unsafe { libc::geteuid() };
    if !metadata.file_type().is_dir() {
        return Err(Error::new(ErrorKind::InvalidInput, "not a directory"));
    }
    if metadata.uid() != owner {
        return Err(Error::new(ErrorKind::PermissionDenied, "owned by another user"));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(Error::new(ErrorKind::PermissionDenied, "open to other users"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "private_dir_tests.rs"]
mod tests;
