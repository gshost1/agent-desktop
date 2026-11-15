/// How a node's Accessible property group (Name, Description, ChildCount and
/// AccessibleId) is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PropertyRead {
    /// Four `Properties.Get` calls in flight together.
    Separate,
    /// One `Properties.GetAll(Accessible)`; a key it refuses or leaves out is
    /// read with its own `Get`.
    Grouped,
    /// `Grouped`, and the group's `Attributes` stands in for `GetAttributes`;
    /// a group without a readable `Attributes` falls back to `GetAttributes`.
    GroupedWithAttributes,
}
