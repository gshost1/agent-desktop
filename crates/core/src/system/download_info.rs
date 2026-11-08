use serde::{Deserialize, Serialize};

/// A browser download a `wait --download` returned: where the file was
/// saved, the name the server suggested, the URL it came from, its final
/// state and its size in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub path: String,
    pub suggested_filename: String,
    pub url: String,
    pub state: String,
    pub bytes: u64,
}
