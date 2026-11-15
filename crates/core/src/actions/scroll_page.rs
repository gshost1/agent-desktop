//! What one page of a scroll area did.

/// The observed outcome of paging a scroll area once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPage {
    /// The page was sent and the area's content moved.
    Moved,
    /// The page was sent but no signal of the area could be read afterwards.
    Unconfirmed,
    /// The area did not move: it already sits at its end that way.
    /// `delivered` says whether a page action was sent anyway.
    AtEnd { delivered: bool },
}
