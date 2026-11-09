use std::path::{Path, PathBuf};

use crate::{AdapterError, AppError, Deadline, ErrorCode, FileLock};

/// A small private file under the agent-desktop state root, such as a
/// registry an adapter keeps between invocations. Reads and writes go through
/// the same private-file checks the ref store uses: the file and its parent
/// must belong to the current user and be closed to everyone else, and a
/// write replaces the file atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFile {
    path: PathBuf,
}

impl StateFile {
    /// The file at `segments` below the configured state root. Each segment
    /// must be a plain file or directory name.
    pub fn under_state_root(segments: &[&str]) -> Result<Self, AdapterError> {
        if segments.is_empty() || segments.iter().any(|segment| !is_plain_name(segment)) {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "A state file path must be one or more plain names",
            ));
        }
        let root =
            crate::storage::state_root::resolve_configured_state_root().map_err(adapter_error)?;
        Ok(Self::at(
            segments
                .iter()
                .fold(root, |path, segment| path.join(segment)),
        ))
    }

    /// A state file at an explicit path, for callers that own its location.
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's bytes, or `None` when it does not exist yet. A file larger
    /// than `max_bytes` is refused rather than truncated.
    pub fn read(&self, max_bytes: u64) -> Result<Option<Vec<u8>>, AdapterError> {
        match crate::storage::private_file::read_private_bounded(&self.path, max_bytes) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_failure("read", &self.path, &error)),
        }
    }

    /// Replaces the file with `bytes`, creating its private parent first.
    pub fn write(&self, bytes: &[u8]) -> Result<(), AdapterError> {
        crate::storage::private_file::write_atomic(&self.path, bytes)
            .map_err(|error| io_failure("write", &self.path, &error))
    }

    /// Holds the file's sibling `.lock` until the guard drops, so a
    /// read-modify-write cannot interleave with another process's.
    pub fn lock(&self, deadline: Deadline, purpose: &str) -> Result<FileLock, AdapterError> {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        FileLock::acquire(&self.path.with_file_name(name), deadline, purpose)
    }
}

fn is_plain_name(segment: &str) -> bool {
    !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(['/', '\\', '\0'])
}

fn adapter_error(error: AppError) -> AdapterError {
    match error {
        AppError::Adapter(error) => error,
        other => AdapterError::internal(other.to_string()),
    }
}

fn io_failure(operation: &str, path: &Path, error: &std::io::Error) -> AdapterError {
    AdapterError::internal(format!(
        "Could not {operation} the state file {}",
        path.display()
    ))
    .with_platform_detail(error.to_string())
}

#[cfg(test)]
#[path = "state_file_tests.rs"]
mod tests;
