use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocatorResolutionMeta {
    pub total_matches: u32,
    pub complete: bool,
    pub selection_complete: bool,
    pub truncated: bool,
    pub roles_present: Vec<String>,
    /// A `--first` pick that prefers whole names stands on a read that
    /// settled the traversal-first match but could hide an earlier
    /// whole-name match: the pick is the best match seen, not a proven one.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub exact_name_unproven: bool,
}
