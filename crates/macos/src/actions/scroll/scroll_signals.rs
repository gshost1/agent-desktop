/// What a page scroll could have moved, read once before the action and again
/// while it settles. Each field is `None` when the area has no such signal,
/// `Some(None)` when the signal exists but could not be read, and
/// `Some(Some(fingerprint))` with the value's bit pattern or bounds hash.
/// `bar` is the scroll bar on the scrolled axis, `content` the frames of the
/// area's first children that are not scroll bars, and `descendant` the frames
/// of the first content child's first children, which move even when the
/// content's own frame is the viewport and its first child is a pinned header.
/// `sampled` counts the elements whose frames were read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScrollSignals {
    pub(crate) bar: Option<Option<u64>>,
    pub(crate) content: Option<Option<u64>>,
    pub(crate) descendant: Option<Option<u64>>,
    pub(crate) sampled: usize,
}

impl ScrollSignals {
    /// `Some(true)` when any signal readable in both reads changed, so the
    /// scroll happened. `Some(false)` only when every signal is readable in
    /// both reads and unchanged, and those signals include the scroll bar's
    /// value or the frames of at least two elements, which proves the action
    /// moved nothing. A single frame is no proof: a content view sized to the
    /// viewport, or a pinned header, stays put while the scroll moves the rest.
    /// `None` when the evidence cannot decide.
    pub(crate) fn movement(self, after: ScrollSignals) -> Option<bool> {
        let pairs = [
            (self.bar, after.bar),
            (self.content, after.content),
            (self.descendant, after.descendant),
        ];
        if pairs
            .iter()
            .any(|pair| matches!(pair, (Some(Some(before)), Some(Some(now))) if before != now))
        {
            return Some(true);
        }
        let mut available = 0;
        for pair in pairs {
            match pair {
                (None, None) => {}
                (Some(Some(_)), Some(Some(_))) => available += 1,
                _ => return None,
            }
        }
        let bar_read = matches!(self.bar, Some(Some(_)));
        (available > 0 && (bar_read || self.sampled >= 2)).then_some(false)
    }
}
