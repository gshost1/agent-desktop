use super::{shortcut_key::ShortcutKey, shortcut_target::ShortcutTarget};

/// The key equivalent one menu item publishes through `AXMenuItemCmdChar`,
/// `AXMenuItemCmdModifiers` and `AXMenuItemCmdVirtualKey`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MenuItemShortcut {
    pub(crate) character: Option<String>,
    pub(crate) modifiers: Option<u32>,
    pub(crate) virtual_key: Option<u16>,
}

impl MenuItemShortcut {
    /// An item without a readable modifier value is never a match: guessing
    /// the command-only default could press an item the chord does not name.
    pub(crate) fn matches(&self, target: &ShortcutTarget) -> bool {
        if self.modifiers != Some(target.modifiers) {
            return false;
        }
        match &target.key {
            ShortcutKey::Character(expected) => self
                .character
                .as_deref()
                .is_some_and(|published| published.to_uppercase() == *expected),
            ShortcutKey::VirtualKey(code) => {
                self.character.as_deref().is_none_or(str::is_empty)
                    && self.virtual_key == Some(*code)
            }
        }
    }
}

#[cfg(test)]
#[path = "menu_item_shortcut_tests.rs"]
mod tests;
