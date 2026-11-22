/// Which incomplete walks a resolution answers with what it observed instead
/// of failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PartialAnswers {
    /// Every incomplete walk fails closed.
    Never,
    /// Every incomplete walk returns its observed matches.
    Always,
    /// A walk the platform could not finish for a reason no retry or larger
    /// budget changes returns its matches when it observed at least one.
    /// Budget limits and transient read failures still fail.
    ObservedMatches,
}

impl PartialAnswers {
    pub(crate) fn on_limit(self) -> bool {
        self == Self::Always
    }

    pub(crate) fn on_unreadable(self, observed_matches: u32) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::ObservedMatches => observed_matches > 0,
        }
    }
}

#[cfg(test)]
#[path = "partial_answers_tests.rs"]
mod tests;
