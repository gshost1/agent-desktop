use super::menu_match::MenuMatch;

/// Everything one bounded pass over the menu bar read. `unread_menus` names
/// each submenu whose items the application did not hand over, by its
/// top-level menu and parent item, so a press over a partial walk says which
/// part it could not compare.
#[derive(Default)]
pub(crate) struct MenuWalk {
    pub(crate) matches: Vec<MenuMatch>,
    pub(crate) unread_menus: Vec<String>,
    pub(crate) visited: usize,
}
