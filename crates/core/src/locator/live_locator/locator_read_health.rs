use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LocatorReadHealth {
    pub cannot_complete: u64,
    pub native_read_failures: u64,
    pub deadline_exhausted: u64,
    /// Elements a walk enumerated that the platform reported destroyed by the
    /// time it read them: the interface changed under the walk.
    pub invalidated_elements: u64,
}
