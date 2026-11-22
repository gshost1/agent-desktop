use super::{LocatorMatch, LocatorResolutionMeta, LocatorStats};
use crate::refs::RefMap;

pub struct LocatorResolution {
    pub matches: Vec<LocatorMatch>,
    pub refmap: Option<RefMap>,
    pub stats: LocatorStats,
    pub meta: LocatorResolutionMeta,
    /// How deep a snapshot cut of the read's root must reach for its
    /// shallowest match to still match there, read only for
    /// [`super::LocatorMaterialization::MatchDepth`].
    pub(crate) match_depth: Option<u8>,
}
