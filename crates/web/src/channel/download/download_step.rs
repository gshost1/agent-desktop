use agent_desktop_core::{AdapterError, DownloadInfo};

use super::download_record::DownloadRecord;

/// What one look at the ledger, the connection's events and the download
/// folder says about the oldest download no wait returned yet.
pub(crate) enum DownloadStep {
    Ready(DownloadInfo),
    Failed(AdapterError),
    Pending(DownloadRecord),
    Missing,
    Idle,
}
