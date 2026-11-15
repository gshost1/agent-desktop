use std::time::Duration;

use super::{UNRESPONSIVE_LIMIT, UnresponsiveSpell};
use crate::wait::deadline::VirtualClock;

#[test]
fn a_spell_ends_only_once_it_reaches_the_limit() {
    let clock = VirtualClock::freeze();
    let mut spell = UnresponsiveSpell::default();

    assert_eq!(spell.record(true), None);
    clock.advance(UNRESPONSIVE_LIMIT - Duration::from_millis(1));
    assert_eq!(spell.record(true), None);
    clock.advance(Duration::from_millis(1));
    assert_eq!(spell.record(true), Some(UNRESPONSIVE_LIMIT));
}

#[test]
fn an_answered_read_starts_the_spell_over() {
    let clock = VirtualClock::freeze();
    let mut spell = UnresponsiveSpell::default();

    assert_eq!(spell.record(true), None);
    clock.advance(UNRESPONSIVE_LIMIT - Duration::from_millis(1));
    assert_eq!(spell.record(false), None);
    clock.advance(Duration::from_millis(1));
    assert_eq!(spell.record(true), None);
    clock.advance(UNRESPONSIVE_LIMIT - Duration::from_millis(1));
    assert_eq!(spell.record(true), None);
}
