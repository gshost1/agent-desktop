/// What a page scroll could have moved, read once before the action and again
/// while it settles. Each field is `None` when the area has no such signal,
/// `Some(None)` when the signal exists but could not be read, and
/// `Some(Some(fingerprint))` with the value's bit pattern or bounds hash.
/// `bar` is the scroll bar on the scrolled axis, `content` the area's first
/// child that is not a scroll bar, and `descendant` that child's first child,
/// which moves even when the content's own frame is the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScrollSignals {
    pub(crate) bar: Option<Option<u64>>,
    pub(crate) content: Option<Option<u64>>,
    pub(crate) descendant: Option<Option<u64>>,
}

impl ScrollSignals {
    /// `Some(true)` when any signal readable in both reads changed, so the
    /// scroll happened. `Some(false)` only when at least one signal exists and
    /// every signal is readable in both reads and unchanged, which proves the
    /// action moved nothing. `None` when the evidence cannot decide.
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
        (available > 0).then_some(false)
    }
}
