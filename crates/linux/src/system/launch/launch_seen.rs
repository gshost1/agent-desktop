use super::off_bus::OffBus;

/// What an owner search learned beyond the registry: nothing, what it saw of
/// a spawned process that stayed off the bus, or that it took the one
/// running instance as the owner after the launcher exited cleanly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Seen {
    #[default]
    Nothing,
    OffBus(OffBus),
    Adopted,
}

impl Seen {
    /// What was seen of a process off the bus; a search that saw nothing of
    /// it saw no window either.
    pub(crate) fn off_bus(self) -> OffBus {
        match self {
            Self::OffBus(seen) => seen,
            Self::Nothing | Self::Adopted => OffBus::Starting,
        }
    }
}
