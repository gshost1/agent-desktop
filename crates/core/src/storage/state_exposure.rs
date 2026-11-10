use std::path::{Path, PathBuf};

/// A private state path that group or other users can reach. It travels
/// inside a `PermissionDenied` io error so the one refusal that `chmod` can
/// repair stays distinguishable from every other permission refusal.
#[derive(Debug)]
pub(crate) struct StateExposure {
    pub(crate) path: Option<PathBuf>,
    pub(crate) directory: bool,
}

impl std::fmt::Display for StateExposure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let subject = if self.directory {
            "private file parent"
        } else {
            "private file"
        };
        write!(formatter, "{subject} is accessible by group or other users")
    }
}

impl std::error::Error for StateExposure {}

pub(crate) fn exposed_directory(path: &Path) -> std::io::Error {
    exposure(Some(path.to_path_buf()), true)
}

pub(crate) fn exposed_file() -> std::io::Error {
    exposure(None, false)
}

/// Names `path` on a file exposure raised where only the open handle was
/// known; any other error passes through unchanged.
pub(crate) fn with_exposed_path(error: std::io::Error, path: &Path) -> std::io::Error {
    match find(&error) {
        Some(found) if found.path.is_none() => exposure(Some(path.to_path_buf()), found.directory),
        _ => error,
    }
}

pub(crate) fn find(error: &std::io::Error) -> Option<&StateExposure> {
    error.get_ref()?.downcast_ref::<StateExposure>()
}

fn exposure(path: Option<PathBuf>, directory: bool) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        StateExposure { path, directory },
    )
}

#[cfg(test)]
#[path = "state_exposure_tests.rs"]
mod tests;
