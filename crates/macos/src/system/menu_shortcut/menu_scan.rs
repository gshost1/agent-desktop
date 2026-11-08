use super::menu_match::MenuMatch;

/// What one bounded pass over the menu bar found. `MenuOpen` ends the pass
/// early: keys go to an open menu, so a chord must not bypass it.
pub(crate) enum MenuScan {
    Complete(Vec<MenuMatch>),
    MenuOpen,
}
