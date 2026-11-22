#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LocatorMaterialization {
    #[default]
    None,
    SelectedMatches,
    FullRefMap,
    /// No refs, as with `None`, but the resolution reports in `match_depth`
    /// how deep a snapshot of the read's root must reach to hold a match, for
    /// a caller that rebuilds the match as a snapshot.
    MatchDepth,
}
