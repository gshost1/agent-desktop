use agent_desktop_core::{KeyCombo, Modifier};

use super::shortcut_key::ShortcutKey;

pub(crate) const AX_MENU_MODIFIER_SHIFT: u32 = 1 << 0;
pub(crate) const AX_MENU_MODIFIER_OPTION: u32 = 1 << 1;
pub(crate) const AX_MENU_MODIFIER_CONTROL: u32 = 1 << 2;
pub(crate) const AX_MENU_MODIFIER_NO_COMMAND: u32 = 1 << 3;
const AX_MENU_MODIFIER_MASK: u32 = AX_MENU_MODIFIER_SHIFT
    | AX_MENU_MODIFIER_OPTION
    | AX_MENU_MODIFIER_CONTROL
    | AX_MENU_MODIFIER_NO_COMMAND;

/// A key chord in the vocabulary of `AXMenuItemCmdModifiers`, where `0` means
/// command alone and command is the one modifier encoded by its absence.
/// Only chords that hold command qualify: plain keys and control or option
/// chords belong to the focused element, not to the menu bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShortcutTarget {
    pub(crate) key: ShortcutKey,
    pub(crate) modifiers: u32,
}

impl ShortcutTarget {
    pub(crate) fn from_combo(combo: &KeyCombo) -> Option<Self> {
        if !combo.modifiers.contains(&Modifier::Meta) {
            return None;
        }
        Some(Self {
            key: shortcut_key(&combo.key)?,
            modifiers: encode_modifiers(&combo.modifiers),
        })
    }
}

/// Reads a published `AXMenuItemCmdModifiers` value. A value with bits outside
/// the documented four is not a shortcut this lookup can compare, so it reads
/// as no shortcut rather than as a guess.
pub(crate) fn decode_menu_modifiers(number: i64) -> Option<u32> {
    let value = u32::try_from(number).ok()?;
    (value & !AX_MENU_MODIFIER_MASK == 0).then_some(value)
}

fn shortcut_key(key: &str) -> Option<ShortcutKey> {
    if let Some(symbol) = crate::input::keyboard::keyboard_map::punctuation(key) {
        return Some(ShortcutKey::Character(symbol.to_string()));
    }
    let mut characters = key.chars();
    if let (Some(character), None) = (characters.next(), characters.next()) {
        return Some(ShortcutKey::Character(character.to_uppercase().collect()));
    }
    crate::input::keyboard::keyboard_map::key_name_to_code(key)
        .ok()
        .map(ShortcutKey::VirtualKey)
}

fn encode_modifiers(modifiers: &[Modifier]) -> u32 {
    let base = if modifiers.contains(&Modifier::Meta) {
        0
    } else {
        AX_MENU_MODIFIER_NO_COMMAND
    };
    modifiers.iter().fold(base, |encoded, modifier| {
        encoded
            | match modifier {
                Modifier::Shift => AX_MENU_MODIFIER_SHIFT,
                Modifier::Alt => AX_MENU_MODIFIER_OPTION,
                Modifier::Ctrl => AX_MENU_MODIFIER_CONTROL,
                Modifier::Meta => 0,
            }
    })
}

#[cfg(test)]
#[path = "shortcut_target_tests.rs"]
mod tests;
