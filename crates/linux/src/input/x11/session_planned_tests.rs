use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::DeliverySemantics;

use super::{CallEnv, run_journaled};
use crate::input::keyboard::keymap_edit::KeymapEdit;
use crate::input::keyboard::text_plan::plan_text;
use crate::input::x11::fake_sink::{FakeSink, Recorded, deadline, events, run, start_keymap};
use crate::input::x11::plan_source::PlanSource;

fn env(journal: Option<crate::input::x11::held_journal::HeldJournal>) -> CallEnv<'static> {
    CallEnv {
        deadline: deadline(5_000),
        journal,
        slot: Arc::default(),
        release_on_signal: false,
        lease: None,
    }
}

#[test]
fn a_planned_run_reads_the_keymap_on_the_connection_that_sends() {
    let fake = FakeSink::new();
    let opens = Arc::new(AtomicUsize::new(0));
    let (sink, counted) = (fake.clone(), Arc::clone(&opens));
    let delivery = run_journaled(
        move || {
            counted.fetch_add(1, Ordering::SeqCst);
            Ok(sink)
        },
        PlanSource::FromKeymap(Box::new(|map| plan_text(map, "ab"))),
        &mut || Ok(()),
        env(None),
    )
    .expect("typed");
    assert_eq!(opens.load(Ordering::SeqCst), 1);
    assert_eq!(delivery.delivered_events, 4);
    assert_eq!(fake.world().queries[..2], ["keymap", "text_state"]);
    assert_eq!(
        events(&fake),
        [
            Recorded::Key(8, true),
            Recorded::Key(8, false),
            Recorded::Key(9, true),
            Recorded::Key(9, false),
        ]
    );
}

#[test]
fn a_planned_run_recovers_its_own_pending_remaps_before_it_reads_the_keymap() {
    use crate::input::{held_input::HeldInput, x11::held_journal::HeldJournal};
    let dir = std::env::temp_dir().join(format!("ad-planned-remap-{}", std::process::id()));
    let fake = FakeSink::new();
    fake.world().keymap.keysyms[8..10].copy_from_slice(&[0x01004f60; 2]);
    let journal = HeldJournal::at(dir.clone(), ":7", std::process::id());
    journal
        .record(
            &HeldInput::default(),
            &[KeymapEdit {
                keycode: 12,
                keysyms: vec![0, 0],
            }],
        )
        .unwrap();
    let sink = fake.clone();
    run_journaled(
        move || Ok(sink),
        PlanSource::FromKeymap(Box::new(|map| {
            assert_eq!(*map, start_keymap(), "planning saw the recovered keymap");
            plan_text(map, "你")
        })),
        &mut || Ok(()),
        env(Some(journal)),
    )
    .expect("typed after recovery");
    assert_eq!(fake.world().keymap, start_keymap());
    assert!(!dir.join(format!("{}.json", std::process::id())).exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_planning_refusal_from_the_worker_sends_nothing() {
    let fake = FakeSink::new();
    let sink = fake.clone();
    let error = run_journaled(
        move || Ok(sink),
        PlanSource::FromKeymap(Box::new(|map| plan_text(map, "a\u{7}"))),
        &mut || Ok(()),
        env(None),
    )
    .expect_err("a control character is refused at planning");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(events(&fake).is_empty());
}

#[test]
fn a_layout_replaced_after_planning_refuses_before_the_first_key() {
    let fake = FakeSink::new();
    let plans = plan_text(&fake.world().keymap, "ab").unwrap();
    let error = run(
        &fake,
        plans,
        &mut || {
            fake.world().keymap.keysyms[0..2].copy_from_slice(&[0x71, 0x51]);
            Ok(())
        },
        deadline(5_000),
        None,
    )
    .expect_err("the planned keycode now types q");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["check"], "keyboard_layout");
    assert_eq!(details["keycode"], 8);
    assert!(events(&fake).is_empty());
}

#[test]
fn a_layout_replaced_between_chunks_stops_the_next_chunk() {
    let fake = FakeSink::new();
    let plans = plan_text(&fake.world().keymap, &"a".repeat(65)).unwrap();
    let mut chunks = 0;
    let error = run(
        &fake,
        plans,
        &mut || {
            chunks += 1;
            if chunks == 3 {
                fake.world().keymap.keysyms[0..2].copy_from_slice(&[0x71, 0x51]);
            }
            Ok(())
        },
        deadline(5_000),
        None,
    )
    .expect_err("the third chunk would type q");
    assert_eq!(events(&fake).len(), 128);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn a_change_to_a_keycode_the_text_does_not_press_lets_it_through() {
    let fake = FakeSink::new();
    let plans = plan_text(&fake.world().keymap, "a").unwrap();
    run(
        &fake,
        plans,
        &mut || {
            fake.world().keymap.keysyms[2..4].copy_from_slice(&[0x71, 0x51]);
            Ok(())
        },
        deadline(5_000),
        None,
    )
    .expect("keycode 8 still types a");
    assert_eq!(events(&fake).len(), 2);
}
