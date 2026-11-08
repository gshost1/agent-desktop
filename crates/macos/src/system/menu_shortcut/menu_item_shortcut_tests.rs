use super::*;
use crate::system::menu_shortcut::shortcut_target::{
    AX_MENU_MODIFIER_NO_COMMAND, AX_MENU_MODIFIER_SHIFT,
};
use agent_desktop_core::{KeyCombo, Modifier};

fn target(key: &str, modifiers: Vec<Modifier>) -> ShortcutTarget {
    ShortcutTarget::from_combo(&KeyCombo {
        key: key.into(),
        modifiers,
    })
    .unwrap()
}

fn item(
    character: Option<&str>,
    modifiers: Option<u32>,
    virtual_key: Option<u16>,
) -> MenuItemShortcut {
    MenuItemShortcut {
        character: character.map(str::to_string),
        modifiers,
        virtual_key,
    }
}

#[test]
fn a_published_character_matches_regardless_of_case() {
    let chord = target("n", vec![Modifier::Meta, Modifier::Shift]);
    assert!(item(Some("N"), Some(AX_MENU_MODIFIER_SHIFT), None).matches(&chord));
    assert!(item(Some("n"), Some(AX_MENU_MODIFIER_SHIFT), None).matches(&chord));
}

#[test]
fn the_same_character_with_other_modifiers_is_a_different_shortcut() {
    let chord = target("g", vec![Modifier::Meta, Modifier::Shift]);
    assert!(!item(Some("G"), Some(0), None).matches(&chord));
    let chord = target("g", vec![Modifier::Meta]);
    assert!(!item(Some("G"), Some(AX_MENU_MODIFIER_SHIFT), None).matches(&chord));
}

#[test]
fn an_item_that_needs_no_command_never_matches_a_command_chord() {
    let chord = target("n", vec![Modifier::Meta]);
    assert!(!item(Some("N"), Some(AX_MENU_MODIFIER_NO_COMMAND), None).matches(&chord));
}

#[test]
fn an_item_without_readable_modifiers_is_not_guessed_to_be_command_only() {
    let chord = target("s", vec![Modifier::Meta]);
    assert!(!item(Some("S"), None, None).matches(&chord));
}

#[test]
fn an_item_without_a_shortcut_never_matches() {
    let chord = target("s", vec![Modifier::Meta]);
    assert!(!MenuItemShortcut::default().matches(&chord));
    assert!(!item(Some(""), Some(0), None).matches(&chord));
}

#[test]
fn a_named_key_matches_the_virtual_key_of_an_item_with_no_character() {
    let chord = target("delete", vec![Modifier::Meta]);
    assert!(item(None, Some(0), Some(51)).matches(&chord));
    assert!(item(Some(""), Some(0), Some(51)).matches(&chord));
    assert!(!item(Some("X"), Some(0), Some(51)).matches(&chord));
    assert!(!item(None, Some(0), Some(117)).matches(&chord));
}

#[test]
fn a_character_chord_ignores_an_items_virtual_key() {
    let chord = target("a", vec![Modifier::Meta]);
    assert!(!item(None, Some(0), Some(0)).matches(&chord));
}
