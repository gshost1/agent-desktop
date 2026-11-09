/// One settle read of the target process's menu surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuRead {
    /// A menu surface absent from the closed baseline is on screen.
    Appeared,
    /// No new menu surface, or the read could not be completed; read again.
    Absent,
    /// The settle window or the operation deadline ended.
    Exhausted,
}
