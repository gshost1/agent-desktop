use std::time::SystemTime;

/// A finished file in a download folder: its name, size and last change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FolderFile {
    pub(crate) name: String,
    pub(crate) bytes: u64,
    pub(crate) modified: SystemTime,
}
