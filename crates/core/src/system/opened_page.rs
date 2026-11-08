use serde::{Deserialize, Serialize};

/// The page `open-url` loaded: its final URL and title, the channel that
/// loaded it, the browser's id for the tab, and whether the URL replaced the
/// shown tab's page (`current_tab`) or went to a new background tab
/// (`new_tab`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenedPage {
    pub url: String,
    pub title: String,
    pub channel: String,
    pub target_id: String,
    pub opened: String,
}
