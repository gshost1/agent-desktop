//! Which build of the binary a process runs: the package version and the identity of the
//! executable file on disk, so a rebuilt or replaced binary never talks to an older daemon.

use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

/// The version and executable-file identity of a running agent-desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildIdentity(String);

impl BuildIdentity {
    /// The identity of this process's own executable, or `None` when it cannot be read.
    pub(crate) fn current() -> Option<Self> {
        let metadata = std::fs::metadata(own_executable()?).ok()?;
        Some(Self::from_parts(env!("CARGO_PKG_VERSION"), &metadata))
    }

    pub(crate) fn from_parts(version: &str, metadata: &std::fs::Metadata) -> Self {
        Self(format!(
            "{version}:{}:{}:{}:{}.{}",
            metadata.dev(),
            metadata.ino(),
            metadata.size(),
            metadata.mtime(),
            metadata.mtime_nsec()
        ))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The file this process was started from. On Linux `/proc/self/exe` keeps naming the original
/// inode after the binary is replaced, so a daemon never reports a newer build than it runs.
fn own_executable() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Some(PathBuf::from("/proc/self/exe"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::current_exe().ok()
    }
}

#[cfg(test)]
#[path = "build_identity_tests.rs"]
mod tests;
