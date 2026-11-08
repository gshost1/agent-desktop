use super::*;

#[test]
fn one_enabled_match_is_pressed() {
    assert_eq!(
        MenuShortcutChoice::choose(&[true]),
        MenuShortcutChoice::Press(0)
    );
}

#[test]
fn a_disabled_duplicate_does_not_block_the_enabled_item() {
    assert_eq!(
        MenuShortcutChoice::choose(&[false, true]),
        MenuShortcutChoice::Press(1)
    );
}

#[test]
fn two_enabled_items_with_one_chord_fall_back_instead_of_guessing() {
    assert_eq!(
        MenuShortcutChoice::choose(&[true, true]),
        MenuShortcutChoice::Ambiguous
    );
    assert_eq!(
        MenuShortcutChoice::choose(&[true, false, true]),
        MenuShortcutChoice::Ambiguous
    );
}

#[test]
fn only_disabled_matches_fall_back() {
    assert_eq!(
        MenuShortcutChoice::choose(&[false, false]),
        MenuShortcutChoice::Disabled
    );
}

#[test]
fn no_match_leaves_the_keyboard_path_alone() {
    assert_eq!(MenuShortcutChoice::choose(&[]), MenuShortcutChoice::NoMatch);
}
