/// The target process's menu surfaces, read before a menu-opening chain and
/// positively closed: the accessibility tree held no open menu and the window
/// server answered. The pop-up menu windows the process already had on screen
/// then are recorded so that only a menu window that appears afterwards counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MenuBaseline {
    pub(crate) menu_windows: Vec<i64>,
}
