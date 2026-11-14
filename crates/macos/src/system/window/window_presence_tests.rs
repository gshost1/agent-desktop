use super::{WindowPresence, retain_published_floating};
use crate::system::window::cg_window::{
    WindowDictionary, WindowRecord, WindowRecordScope, records_from_dictionaries,
};
use crate::system::window::window_ax_titles::WindowAxTitles;
use agent_desktop_core::AdapterError;

fn record(pid: i32, window_number: i64, presence: WindowPresence) -> WindowRecord {
    WindowRecord {
        app_name: "Fixture".into(),
        pid,
        title: None,
        window_number,
        bounds: agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        presence,
        process_instance: Some(format!("instance-{pid}")),
    }
}

fn published(numbers: &[i64]) -> WindowAxTitles {
    let mut titles = WindowAxTitles::default();
    for number in numbers {
        titles.record(*number, None);
    }
    titles
}

fn dictionary(window_number: i64, layer: i64, onscreen: bool) -> WindowDictionary {
    use core_foundation::{
        base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, number::CFNumber,
        string::CFString,
    };

    let bounds = CFDictionary::from_CFType_pairs(&[
        (CFString::new("X"), CFNumber::from(0).as_CFType()),
        (CFString::new("Y"), CFNumber::from(0).as_CFType()),
        (CFString::new("Width"), CFNumber::from(300).as_CFType()),
        (CFString::new("Height"), CFNumber::from(200).as_CFType()),
    ]);
    CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("kCGWindowLayer"),
            CFNumber::from(layer).as_CFType(),
        ),
        (
            CFString::new("kCGWindowOwnerPID"),
            CFNumber::from(10).as_CFType(),
        ),
        (
            CFString::new("kCGWindowNumber"),
            CFNumber::from(window_number).as_CFType(),
        ),
        (
            CFString::new("kCGWindowOwnerName"),
            CFString::new("Fixture").as_CFType(),
        ),
        (
            CFString::new("kCGWindowIsOnscreen"),
            CFBoolean::from(onscreen).as_CFType(),
        ),
        (CFString::new("kCGWindowBounds"), bounds.as_CFType()),
    ])
}

#[test]
fn normal_level_windows_keep_their_onscreen_state() {
    assert_eq!(
        WindowPresence::classify(0, || Ok(true)).unwrap(),
        Some(WindowPresence::Onscreen)
    );
    assert_eq!(
        WindowPresence::classify(0, || Ok(false)).unwrap(),
        Some(WindowPresence::Offscreen)
    );
}

#[test]
fn an_onscreen_panel_level_window_is_floating_and_a_hidden_one_is_not_listed() {
    for layer in [3, 8, 19] {
        assert_eq!(
            WindowPresence::classify(layer, || Ok(true)).unwrap(),
            Some(WindowPresence::Floating)
        );
        assert_eq!(WindowPresence::classify(layer, || Ok(false)).unwrap(), None);
    }
}

#[test]
fn dock_menu_and_overlay_levels_are_never_listed_or_read() {
    for layer in [-1, 20, 24, 25, 101, 1000] {
        let presence = WindowPresence::classify(layer, || {
            Err(AdapterError::internal("onscreen must not be read"))
        });
        assert_eq!(presence.unwrap(), None);
    }
}

#[test]
fn the_inventory_lists_an_onscreen_panel_as_floating() {
    let records = records_from_dictionaries(
        vec![
            dictionary(7, 0, true),
            dictionary(8, 3, true),
            dictionary(9, 3, false),
            dictionary(10, 25, true),
        ],
        WindowRecordScope::Pid(10),
    )
    .unwrap();

    let listed = records
        .iter()
        .map(|record| (record.window_number, record.presence))
        .collect::<Vec<_>>();
    assert_eq!(
        listed,
        vec![(7, WindowPresence::Onscreen), (8, WindowPresence::Floating)]
    );
}

#[test]
fn a_floating_window_stays_only_when_its_process_publishes_it() {
    let mut records = vec![
        record(10, 7, WindowPresence::Onscreen),
        record(10, 8, WindowPresence::Floating),
        record(10, 9, WindowPresence::Floating),
        record(20, 11, WindowPresence::Offscreen),
    ];
    let mut reads = Vec::new();

    retain_published_floating(&mut records, |pid| {
        reads.push(pid);
        Ok(published(&[7, 8]))
    });

    let kept = records
        .iter()
        .map(|record| record.window_number)
        .collect::<Vec<_>>();
    assert_eq!(kept, vec![7, 8, 11]);
    assert_eq!(reads, vec![10]);
}

#[test]
fn an_unreadable_process_loses_only_its_floating_windows() {
    let mut records = vec![
        record(10, 7, WindowPresence::Onscreen),
        record(10, 8, WindowPresence::Floating),
    ];

    retain_published_floating(&mut records, |_| Err(AdapterError::timeout("busy")));

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].window_number, 7);
}

#[test]
fn an_inventory_without_floating_windows_reads_no_process() {
    let mut records = vec![record(10, 7, WindowPresence::Onscreen)];

    retain_published_floating(&mut records, |_| -> Result<WindowAxTitles, AdapterError> {
        panic!("no floating record, no accessibility read")
    });

    assert_eq!(records.len(), 1);
}
