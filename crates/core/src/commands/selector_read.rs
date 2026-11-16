use super::selector_hit::SelectorHit;

/// What one read of a selector wait saw.
pub(crate) enum SelectorRead {
    /// The read could not settle whether the selector is there.
    Incomplete,
    /// A complete read found no match.
    Absent,
    Hit(SelectorHit),
}
