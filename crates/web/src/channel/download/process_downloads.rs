use serde::{Deserialize, Serialize};

use super::download_record::DownloadRecord;

/// The downloads the ledger keeps for one browser process, oldest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProcessDownloads {
    pub(crate) pid: u32,
    pub(crate) process_instance: String,
    #[serde(default)]
    pub(crate) records: Vec<DownloadRecord>,
}
