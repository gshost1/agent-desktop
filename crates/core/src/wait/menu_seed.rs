use crate::ProcessIdentity;

/// A menu reading taken before the action a following `wait --menu-closed`
/// waits after, the way `batch` seeds `wait --event` with a pre-action
/// baseline. A menu open before the action is the evidence that the action
/// closes it rather than opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSeed {
    pub process: ProcessIdentity,
    pub open: bool,
}
