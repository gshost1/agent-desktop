use serde::{Deserialize, Serialize};

use super::download_record::DownloadRecord;

/// The downloads the ledger keeps for one browser process, oldest first,
/// and the names in its download folder a wait must not return: files that
/// were there when the browser launched and files a wait already returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProcessDownloads {
    pub(crate) pid: u32,
    pub(crate) process_instance: String,
    #[serde(default)]
    pub(crate) records: Vec<DownloadRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) claimed: Vec<String>,
}

impl ProcessDownloads {
    /// Marks a file in the download folder as not to be returned again.
    pub(crate) fn claim(&mut self, name: &str) {
        if !self.claimed.iter().any(|claimed| claimed == name) {
            self.claimed.push(name.to_owned());
        }
    }

    pub(crate) fn is_claimed(&self, name: &str) -> bool {
        self.claimed.iter().any(|claimed| claimed == name)
    }
}
