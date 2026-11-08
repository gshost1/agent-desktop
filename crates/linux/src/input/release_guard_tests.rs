use agent_desktop_core::{AdapterError, DeliverySemantics};

use super::ReleaseGuard;
use crate::input::keyboard::keymap_edit::KeymapEdit;
use crate::input::test_scratch::Scratch;
use crate::input::x11::fake_sink::{FakeSink, Recorded, start_keymap};
use crate::input::x11::held_journal::HeldJournal;
use crate::input::x11::sink_error::SinkError;

fn without_round_trips(sink: &FakeSink) -> Vec<Recorded> {
    sink.recorded()
        .into_iter()
        .filter(|event| *event != Recorded::RoundTrip)
        .collect()
}

fn remap() -> KeymapEdit {
    KeymapEdit {
        keycode: 12,
        keysyms: vec![0x1234, 0x1234],
    }
}

fn restore() -> KeymapEdit {
    KeymapEdit {
        keycode: 12,
        keysyms: vec![0, 0],
    }
}

#[test]
fn failed_restores_remain_recoverable_until_acknowledged() {
    let dir = Scratch::adopt(
        std::env::temp_dir().join(format!("ad-restore-retry-{}", std::process::id())),
    );
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.journal_to(Some(HeldJournal::at(dir.to_path_buf(), ":7", 111)));
    let observer = HeldJournal::at(dir.to_path_buf(), ":7", 222);
    guard.edit_keymap(&remap()).unwrap();
    fake.world().fail_on_event = Some(2);
    guard
        .edit_keymap(&restore())
        .expect_err("planned restore fails");
    fake.world().fail_on_event = Some(3);
    guard
        .release_everything()
        .expect_err("cleanup restore also fails");
    assert_eq!(observer.stale_entries(&|_, _| false)[0].remaps, [restore()]);
    assert_ne!(fake.world().keymap, start_keymap());
    guard.release_everything().expect("recovery is retried");
    assert_eq!(fake.world().keymap, start_keymap());
    assert!(observer.stale_entries(&|_, _| false).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn keys_are_released_in_the_reverse_order_they_were_pressed() {
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    {
        let mut guard = ReleaseGuard::new(&mut sink);
        guard.key(37, true).expect("ctrl");
        guard.key(50, true).expect("shift");
        guard.key(38, true).expect("a");
    }

    let events = without_round_trips(&fake);
    assert_eq!(
        events[3..],
        [
            Recorded::Key(38, false),
            Recorded::Key(50, false),
            Recorded::Key(37, false)
        ]
    );
}

#[test]
fn a_remap_is_restored_when_the_guard_is_dropped_without_finishing() {
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    {
        let mut guard = ReleaseGuard::new(&mut sink);
        guard.edit_keymap(&remap()).expect("remap");
    }

    assert_eq!(fake.world().keymap, start_keymap());
}

#[test]
fn a_restoring_edit_inside_the_plan_is_not_applied_twice() {
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.edit_keymap(&remap()).expect("remap");
    guard.edit_keymap(&restore()).expect("restore");
    guard.release_everything().expect("finish");

    let edits = without_round_trips(&fake)
        .into_iter()
        .filter(|event| matches!(event, Recorded::Keymap(_)))
        .count();
    assert_eq!(edits, 2);
}

#[test]
fn a_press_is_journaled_before_it_is_sent_and_cleared_when_released() {
    let dir = Scratch::adopt(
        std::env::temp_dir().join(format!("ad-guard-journal-{}", std::process::id())),
    );
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.journal_to(Some(HeldJournal::at(dir.to_path_buf(), ":7", 111)));
    guard.key(38, true).expect("press");
    let observer = HeldJournal::at(dir.to_path_buf(), ":7", 222);

    assert_eq!(observer.stale_entries(&|_, _| false).len(), 1);
    guard.key(38, false).expect("release");
    assert!(observer.stale_entries(&|_, _| false).is_empty());
}

#[test]
fn a_press_whose_journal_cannot_be_written_is_not_sent() {
    let dir = Scratch::adopt(
        std::env::temp_dir().join(format!("ad-guard-blocked-{}", std::process::id())),
    );
    std::fs::write(&dir, "a file where the directory belongs").expect("file");
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.journal_to(Some(HeldJournal::at(dir.to_path_buf(), ":7", 111)));

    let error = guard.key(38, true).expect_err("the press is refused");
    drop(guard);

    assert!(matches!(error, SinkError::Journal(_)));
    assert!(without_round_trips(&fake).is_empty());
}

#[test]
fn a_remap_is_journaled_with_the_original_keysyms_before_it_is_sent() {
    let dir =
        Scratch::adopt(std::env::temp_dir().join(format!("ad-guard-remap-{}", std::process::id())));
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.journal_to(Some(HeldJournal::at(dir.to_path_buf(), ":7", 111)));
    guard.edit_keymap(&remap()).expect("remap");
    let observer = HeldJournal::at(dir.to_path_buf(), ":7", 222);

    let pending = observer.stale_entries(&|_, _| false);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].remaps, [restore()]);
    guard.release_everything().expect("finish");
    assert!(observer.stale_entries(&|_, _| false).is_empty());
}

#[test]
fn an_abort_after_every_key_came_back_up_posts_nothing_and_claims_no_emergency_release() {
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.key(38, true).expect("press");
    guard.key(38, false).expect("release");
    let before = without_round_trips(&fake);

    let error = guard.abort(AdapterError::timeout("between characters"), "inspect");

    assert_eq!(without_round_trips(&fake), before);
    let details = error.details.expect("delivered input reports evidence");
    assert_eq!(details["delivered_events"], 2);
    assert!(details.get("emergency_release_posted").is_none());
    assert!(details.get("emergency_release_acknowledged").is_none());
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn an_abort_while_a_key_is_held_releases_exactly_that_key_and_reports_the_round_trip() {
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.key(37, true).expect("ctrl");
    guard.key(38, true).expect("a down");
    guard.key(38, false).expect("a up");
    let before = without_round_trips(&fake).len();

    let error = guard.abort(AdapterError::timeout("mid chord"), "inspect");

    assert_eq!(
        without_round_trips(&fake)[before..],
        [Recorded::Key(37, false)]
    );
    let details = error.details.expect("evidence");
    assert_eq!(details["delivered_events"], 3);
    assert_eq!(details["emergency_release_posted"], true);
    assert_eq!(details["emergency_release_acknowledged"], true);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn an_abort_whose_release_fails_reports_it_unacknowledged() {
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    guard.key(37, true).expect("ctrl");
    fake.world().fail_on_event = Some(2);

    let error = guard.abort(AdapterError::timeout("mid chord"), "inspect");

    let details = error.details.expect("evidence");
    assert_eq!(details["emergency_release_posted"], true);
    assert_eq!(details["emergency_release_acknowledged"], false);
}

#[test]
fn an_edit_to_a_keycode_whose_symbols_cannot_be_read_is_refused_unsent() {
    let fake = FakeSink::new();
    let mut sink = fake.clone();
    let mut guard = ReleaseGuard::new(&mut sink);
    let unrestorable = KeymapEdit {
        keycode: 200,
        keysyms: vec![0x1234, 0x1234],
    };

    let refused = guard
        .edit_keymap(&unrestorable)
        .expect_err("an edit that cannot be restored is not sent");
    let error = guard.abort(refused.into_adapter(), "inspect");

    assert!(without_round_trips(&fake).is_empty());
    assert_eq!(fake.world().keymap, start_keymap());
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}
