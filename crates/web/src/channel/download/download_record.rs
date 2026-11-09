use serde::{Deserialize, Serialize};

use super::download_state::DownloadState;

/// One download a browser announced over DevTools, as the download ledger
/// keeps it between invocations. `path` is the saved file once it is known;
/// `delivered` marks a download a `wait --download` already returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DownloadRecord {
    pub(crate) guid: String,
    pub(crate) url: String,
    pub(crate) suggested_filename: String,
    pub(crate) state: DownloadState,
    pub(crate) bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(default)]
    pub(crate) delivered: bool,
}
