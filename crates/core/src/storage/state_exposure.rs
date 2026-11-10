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

/// Names `path` on a file exposure raised where only the open handle was
/// known; any other error passes through unchanged.
pub(crate) fn with_exposed_path(error: std::io::Error, path: &Path) -> std::io::Error {
    match find(&error) {
        Some(found) if found.path.is_none() => exposed(Some(path), found.directory),
        _ => error,
    }
}

pub(crate) fn find(error: &std::io::Error) -> Option<&StateExposure> {
    error.get_ref()?.downcast_ref::<StateExposure>()
}

/// Raises the refusal for `path`, a directory or a file, that group or
/// other users can reach. `None` stands for a file known only by its handle.
pub(crate) fn exposed(path: Option<&Path>, directory: bool) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        StateExposure {
            path: path.map(Path::to_path_buf),
            directory,
        },
    )
}

#[cfg(test)]
#[path = "state_exposure_tests.rs"]
mod tests;
