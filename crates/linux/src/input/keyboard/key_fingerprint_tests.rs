use super::KeyFingerprint;
use crate::input::keyboard::keymap::Keymap;

#[test]
fn an_unchanged_keymap_matches_its_fingerprint() {
    let map = Keymap::sample();
    let mut planned = KeyFingerprint::default();
    planned.record(&map, 10);
    planned.record(&map, 10);
    planned.record(&map, 8);
    assert_eq!(planned.changed_in(&map), None);
}

#[test]
fn a_moved_keysym_on_a_planned_keycode_is_named() {
    let map = Keymap::sample();
    let mut planned = KeyFingerprint::default();
    planned.record(&map, 10);
    let mut replaced = map.clone();
    replaced.keysyms[8..10].copy_from_slice(&[0x71, 0x51]);
    assert_eq!(planned.changed_in(&replaced), Some(10));
}

#[test]
fn a_change_on_a_keycode_the_plan_does_not_press_is_ignored() {
    let map = Keymap::sample();
    let mut planned = KeyFingerprint::default();
    planned.record(&map, 10);
    let mut replaced = map.clone();
    replaced.keysyms[12..14].copy_from_slice(&[0x32, 0x40]);
    assert_eq!(planned.changed_in(&replaced), None);
}
