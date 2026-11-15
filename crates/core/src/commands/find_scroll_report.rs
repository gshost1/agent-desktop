//! How far a scrolling find paged before it stopped.

/// What a scrolling find did to the view, returned beside its matches.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct ScrollSearchReport {
    /// Pages sent that moved the area or could not be confirmed either way.
    pub(super) pages: u32,
    /// Whether the area's content was seen to move: the view is left there.
    pub(super) view_moved: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub(super) unconfirmed_pages: u32,
    pub(super) directions: Vec<&'static str>,
    /// `found`, `both_ends` or `page_cap`.
    pub(super) stopped: &'static str,
}

impl ScrollSearchReport {
    pub(super) fn may_have_moved(&self) -> bool {
        self.view_moved || self.unconfirmed_pages > 0
    }
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}
