//! One region a snapshot's contents point to.

use serde::{Deserialize, Serialize};

/// Where a list or bar starts in a snapshot, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRegion {
    /// Index of the region's node in tree order, which is its index in
    /// `data.lines`.
    pub line: usize,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// The items the list has loaded, which a lazily loaded list keeps to
    /// the rows near its viewport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<usize>,
    /// The list's scroll area can still scroll, so it may hold items this
    /// snapshot does not show.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}
