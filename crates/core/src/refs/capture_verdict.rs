use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de::Visitor};

/// What the capture a ref was published from proved about its identity, which
/// decides before anything is read from the application whether a tokenless ref
/// may be resolved, and which refusal it gets when it may not.
///
/// The refmap stores it as `capture`, one of the verdict names. A name this
/// build does not know reads as `Unknown`, so a refmap from a newer build
/// refuses that entry instead of failing to load. A refmap written before the
/// verdict existed stores the boolean `unique_at_capture` instead: `true` reads
/// as `Unique`, and `false` or no flag reads as `Ambiguous`, because such a file
/// cannot tell the other verdicts apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureVerdict {
    /// One complete read of the whole window found exactly one element
    /// matching the entry's full predicate, and it is the element the entry
    /// was built from.
    Unique,
    /// Two or more observed elements matched the entry's full predicate, so no
    /// snapshot can tell them apart. A twin seen in a partial read is still a
    /// twin.
    Ambiguous,
    /// No twin was seen, but the capture was not a complete read of the whole
    /// window, or no longer held the path a drill recorded, so it could not
    /// count the entry's predicate; a capture that reads the whole window, with
    /// a larger timeout, can.
    Incomplete,
    /// The capture could not establish the entry's identity: an observed
    /// element left a field the entry records unread, the entry's predicate
    /// matched one other element and not its own, or no capture judged the
    /// entry at all. Nothing says a new snapshot would read it differently.
    #[default]
    Unknown,
}

impl CaptureVerdict {
    /// Whether the capture proved the entry unique.
    pub fn is_unique(self) -> bool {
        self == Self::Unique
    }

    /// The verdict a refmap written before the verdict existed recorded with
    /// its `unique_at_capture` flag, where no flag meant `false`.
    pub(crate) fn from_legacy_flag(unique: bool) -> Self {
        if unique {
            Self::Unique
        } else {
            Self::Ambiguous
        }
    }
}

struct VerdictName;

impl Visitor<'_> for VerdictName {
    type Value = CaptureVerdict;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a capture verdict name")
    }

    fn visit_str<E: serde::de::Error>(self, name: &str) -> Result<CaptureVerdict, E> {
        Ok(match name {
            "unique" => CaptureVerdict::Unique,
            "ambiguous" => CaptureVerdict::Ambiguous,
            "incomplete" => CaptureVerdict::Incomplete,
            _ => CaptureVerdict::Unknown,
        })
    }
}

impl<'de> Deserialize<'de> for CaptureVerdict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(VerdictName)
    }
}

#[cfg(test)]
#[path = "capture_verdict_tests.rs"]
mod tests;
