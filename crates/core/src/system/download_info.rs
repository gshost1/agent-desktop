use serde::{Deserialize, Serialize};

/// A browser download a `wait --download` returned: where the file was
/// saved, the name the server suggested, the URL it came from, its final
/// state and its size in bytes. `url` is absent for a download found only in
/// the download folder, whose start no DevTools connection saw, and
/// `suggested_filename` is then the saved file's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub path: String,
    pub suggested_filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub state: String,
    pub bytes: u64,
}
