use serde::{Deserialize, Serialize};

/// Where one browser download stands. `Canceled` is the browser's own
/// report; `Interrupted` is a download whose partial file vanished without a
/// finished file taking its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DownloadState {
    InProgress,
    Completed,
    Canceled,
    Interrupted,
}

impl DownloadState {
    /// The state a `Browser.downloadProgress` event names.
    pub(crate) fn from_cdp(state: &str) -> Option<Self> {
        match state {
            "inProgress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "canceled" => Some(Self::Canceled),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Canceled => "canceled",
            Self::Interrupted => "interrupted",
        }
    }
}
