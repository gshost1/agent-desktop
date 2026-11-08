use super::*;

fn record(pid: i32, window_number: i64, title: Option<&str>) -> WindowRecord {
    WindowRecord {
        app_name: "AgentDeskFixture".into(),
        pid,
        title: title.map(str::to_owned),
        window_number,
        bounds: agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        visible: true,
        process_instance: Some(format!("{pid}:1")),
    }
}

fn walked(entries: &[(i64, &str)]) -> WindowAxState {
    WindowAxState {
        focused: None,
        minimized_by_id: FxHashMap::default(),
        accessible_window_ids: entries.iter().map(|(number, _)| *number).collect(),
        title_by_id: entries
            .iter()
            .map(|(number, title)| (*number, (*title).to_owned()))
            .collect(),
    }
}

#[test]
fn core_graphics_title_is_kept_when_accessibility_reports_another() {
    let mut record = record(10, 1, Some("CG Title"));

    join_ax_title(&mut record, Some(&walked(&[(1, "AX Title")])));

    assert_eq!(record.display_title(), "CG Title");
}

#[test]
fn missing_core_graphics_title_takes_the_accessibility_title() {
    let mut record = record(10, 1, None);

    join_ax_title(&mut record, Some(&walked(&[(1, "AgentDesk Fixture")])));

    let info = record.into_window_info(false, None, true).unwrap();
    assert_eq!(info.title, "AgentDesk Fixture");
}

#[test]
fn window_without_any_title_falls_back_to_the_app_name() {
    let mut other_window = record(10, 1, None);
    let mut unprobed_owner = record(20, 2, None);

    join_ax_title(&mut other_window, Some(&walked(&[(2, "Other Window")])));
    join_ax_title(&mut unprobed_owner, None);

    for record in [other_window, unprobed_owner] {
        let info = record.into_window_info(false, None, true).unwrap();
        assert_eq!(info.title, "AgentDeskFixture");
    }
}

#[test]
fn probes_ask_only_for_untitled_windows_and_skip_failed_owners() {
    let records = vec![
        record(10, 1, Some("Titled")),
        record(10, 2, None),
        record(20, 3, None),
    ];
    let mut requested = Vec::new();

    let states = probe_owners_with(&records, &[10, 20], |pid, untitled| {
        requested.push((pid, untitled.clone()));
        if pid == 20 {
            return Err(AdapterError::timeout("busy"));
        }
        Ok(walked(&[(1, "AX One"), (2, "AX Two")]))
    });

    assert_eq!(
        requested,
        vec![
            (10, FxHashSet::from_iter([2])),
            (20, FxHashSet::from_iter([3]))
        ]
    );
    assert!(states.contains_key(&10));
    assert!(!states.contains_key(&20));
}

#[test]
fn empty_core_graphics_window_name_is_no_title() {
    use core_foundation::{
        base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, number::CFNumber,
        string::CFString,
    };

    let bounds = CFDictionary::from_CFType_pairs(&[
        (CFString::new("X"), CFNumber::from(0).as_CFType()),
        (CFString::new("Y"), CFNumber::from(0).as_CFType()),
        (CFString::new("Width"), CFNumber::from(100).as_CFType()),
        (CFString::new("Height"), CFNumber::from(100).as_CFType()),
    ]);
    let dictionary = CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("kCGWindowLayer"),
            CFNumber::from(0).as_CFType(),
        ),
        (
            CFString::new("kCGWindowOwnerPID"),
            CFNumber::from(10).as_CFType(),
        ),
        (
            CFString::new("kCGWindowNumber"),
            CFNumber::from(7).as_CFType(),
        ),
        (
            CFString::new("kCGWindowOwnerName"),
            CFString::new("AgentDeskFixture").as_CFType(),
        ),
        (
            CFString::new("kCGWindowName"),
            CFString::new("").as_CFType(),
        ),
        (
            CFString::new("kCGWindowIsOnscreen"),
            CFBoolean::true_value().as_CFType(),
        ),
        (CFString::new("kCGWindowBounds"), bounds.as_CFType()),
    ]);

    let records = crate::system::window::cg_window::records_from_dictionaries(
        vec![dictionary],
        crate::system::window::cg_window::WindowRecordScope::Pid(10),
    )
    .unwrap();

    assert_eq!(records[0].title, None);
    assert_eq!(records[0].display_title(), "AgentDeskFixture");
}
