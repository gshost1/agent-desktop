use serde::{Deserialize, Serialize};

/// The page `open-url` loaded: its final URL and title, the channel that
/// loaded it, and the browser's id for the tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenedPage {
    pub url: String,
    pub title: String,
    pub channel: String,
    pub target_id: String,
}
