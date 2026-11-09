use super::*;

fn combo(key: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: key.into(),
        modifiers,
    }
}

#[test]
fn only_chords_that_hold_command_reach_the_menu_bar() {
    for modifiers in [
        Vec::new(),
        vec![Modifier::Shift],
        vec![Modifier::Ctrl],
        vec![Modifier::Alt, Modifier::Shift],
    ] {
        assert_eq!(
            ShortcutTarget::from_combo(&combo("n", modifiers.clone())),
            None,
            "{modifiers:?}"
        );
    }
    for key in ["return", "escape", "a", "tab"] {
        assert_eq!(ShortcutTarget::from_combo(&combo(key, Vec::new())), None);
    }
}

#[test]
fn command_alone_encodes_as_zero() {
    let target = ShortcutTarget::from_combo(&combo("s", vec![Modifier::Meta])).unwrap();
    assert_eq!(target.modifiers, 0);
    assert_eq!(target.key, ShortcutKey::Character("S".into()));
}

#[test]
fn extra_modifiers_set_their_bits_and_keep_command_implicit() {
    let target =
        ShortcutTarget::from_combo(&combo("n", vec![Modifier::Meta, Modifier::Shift])).unwrap();
    assert_eq!(target.modifiers, AX_MENU_MODIFIER_SHIFT);
    let target = ShortcutTarget::from_combo(&combo(
        "w",
        vec![Modifier::Meta, Modifier::Alt, Modifier::Ctrl],
    ))
    .unwrap();
    assert_eq!(
        target.modifiers,
        AX_MENU_MODIFIER_OPTION | AX_MENU_MODIFIER_CONTROL
    );
    assert_eq!(target.modifiers & AX_MENU_MODIFIER_NO_COMMAND, 0);
}

#[test]
fn punctuation_matches_by_symbol_whichever_way_it_was_named() {
    for key in ["comma", ","] {
        let target = ShortcutTarget::from_combo(&combo(key, vec![Modifier::Meta])).unwrap();
        assert_eq!(target.key, ShortcutKey::Character(",".into()));
    }
}

#[test]
fn named_keys_match_by_virtual_key_code() {
    let target = ShortcutTarget::from_combo(&combo("delete", vec![Modifier::Meta])).unwrap();
    assert_eq!(target.key, ShortcutKey::VirtualKey(51));
    let target = ShortcutTarget::from_combo(&combo("f5", vec![Modifier::Meta])).unwrap();
    assert_eq!(target.key, ShortcutKey::VirtualKey(96));
}

#[test]
fn an_unknown_key_name_is_not_a_shortcut() {
    assert_eq!(
        ShortcutTarget::from_combo(&combo("nosuchkey", vec![Modifier::Meta])),
        None
    );
}

#[test]
fn published_modifiers_outside_the_documented_bits_read_as_no_shortcut() {
    assert_eq!(decode_menu_modifiers(0), Some(0));
    assert_eq!(
        decode_menu_modifiers(i64::from(AX_MENU_MODIFIER_NO_COMMAND)),
        Some(AX_MENU_MODIFIER_NO_COMMAND)
    );
    assert_eq!(decode_menu_modifiers(-1), None);
    assert_eq!(decode_menu_modifiers(1 << 8), None);
    assert_eq!(decode_menu_modifiers(i64::from(u32::MAX) + 1), None);
}
