use serde::{Deserialize, Deserializer, Serialize, de::Error};

/// What the capture a ref was published from proved about its identity, which
/// decides before anything is read from the application whether a tokenless ref
/// may be resolved, and which refusal it gets when it may not.
///
/// The refmap stores it as `capture`. A refmap written before the verdict
/// existed stores the flag `unique_at_capture` instead: `true` reads as
/// `Unique`, and `false` or no flag reads as `Ambiguous`, because such a file
/// cannot tell the other verdicts apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureVerdict {
    /// One complete read of the whole window found exactly one element
    /// matching the entry's full predicate, and it is the element the entry
    /// was built from.
    Unique,
    /// Two or more observed elements matched the entry's full predicate, so no
    /// snapshot can tell them apart. Also every entry no capture proof judged.
    #[default]
    Ambiguous,
    /// The capture was not a complete read of the whole window, so it could
    /// not count the entry's predicate; a capture that reads the whole window
    /// can.
    Incomplete,
    /// The whole window was read, but an observed element left a field the
    /// entry records unread, or the entry could not be tied to the element it
    /// was built from, so the count cannot exclude a twin.
    Unknown,
}

impl CaptureVerdict {
    /// Whether the capture proved the entry unique.
    pub fn is_unique(self) -> bool {
        self == Self::Unique
    }

    pub(crate) fn is_ambiguous(&self) -> bool {
        *self == Self::Ambiguous
    }
}

impl<'de> Deserialize<'de> for CaptureVerdict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Bool(true) => Ok(Self::Unique),
            serde_json::Value::Bool(false) => Ok(Self::Ambiguous),
            serde_json::Value::String(verdict) => match verdict.as_str() {
                "unique" => Ok(Self::Unique),
                "ambiguous" => Ok(Self::Ambiguous),
                "incomplete" => Ok(Self::Incomplete),
                "unknown" => Ok(Self::Unknown),
                other => Err(D::Error::unknown_variant(
                    other,
                    &["unique", "ambiguous", "incomplete", "unknown"],
                )),
            },
            other => Err(D::Error::custom(format!(
                "capture verdict must be a string or a legacy boolean, found {other}"
            ))),
        }
    }
}

#[cfg(test)]
#[path = "capture_verdict_tests.rs"]
mod tests;
