use std::path::PathBuf;

use agent_desktop_core::DownloadScope;

/// The folder `launch --cdp --download-dir` named for a browser's
/// downloads, and how the browser keeps saving there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFolder {
    pub path: PathBuf,
    pub scope: DownloadScope,
}

impl DownloadFolder {
    /// A folder only DevTools connections point downloads at, as for a
    /// registry entry written before the scope was recorded.
    pub fn per_connection(path: PathBuf) -> Self {
        Self {
            path,
            scope: DownloadScope::Connection,
        }
    }
}
