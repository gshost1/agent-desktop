use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RefGeometry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<crate::Rect>,
    pub bounds_hash: Option<u64>,
    /// The frame `bounds` is measured in. Absent means a screen rectangle,
    /// which older refmaps stored without a tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds_space: Option<crate::BoundsSpace>,
    /// Hash of the snapshot's screen-space rectangle when `bounds` is stored
    /// in window coordinates (`BoundsSpace::Window`). Absent for screen-space
    /// refs and when the snapshot could not prove screen coordinates.
    #[serde(default = "nonempty_screen_hash")]
    pub screen_bounds_hash: Option<u64>,
}

fn nonempty_screen_hash() -> Option<u64> { Some(0) }
