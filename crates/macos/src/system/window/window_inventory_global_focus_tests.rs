use super::*;

#[test]
fn frontmost_owner_reporting_no_focused_window_lists_windows_unfocused() {
    let owners = owners(&[(10, 100.0), (20, 200.0)], Some(10));
    let records = vec![record(10, 1, "Front"), record(20, 2, "Background")];

    let windows = assemble_global_windows(records, &owners, false, |_| Ok(ax_state(None))).unwrap();

    assert_eq!(windows.len(), 2);
    assert!(windows.iter().all(|window| !window.state.is_focused));
}

#[test]
fn focused_only_is_empty_when_the_frontmost_owner_reports_no_focused_window() {
    let owners = owners(&[(10, 100.0)], Some(10));

    let windows = assemble_global_windows(vec![record(10, 1, "Front")], &owners, true, |_| {
        Ok(ax_state(None))
    })
    .unwrap();

    assert!(windows.is_empty());
}

#[test]
fn focused_window_without_a_core_graphics_title_takes_the_focused_ax_title() {
    let owners = owners(&[(10, 100.0)], Some(10));
    let mut untitled = record(10, 1, "");
    untitled.title = None;

    let windows = assemble_global_windows(vec![untitled], &owners, true, |_| {
        Ok(ax_state(Some((Some("AX Title".into()), Some(1)))))
    })
    .unwrap();

    assert_eq!(windows[0].title, "AX Title");
}

#[test]
fn untitled_records_filled_from_probes_join_focus_by_title() {
    let owners = owners(&[(10, 100.0)], Some(10));
    let mut records = vec![record(10, 1, ""), record(10, 2, "")];
    records.iter_mut().for_each(|record| record.title = None);
    let probes = crate::system::window::window_title::probe_owners_with(&records, |_, _| {
        Ok(crate::system::window::window_title::WindowTitleProbe {
            accessible_window_ids: [1, 2].into_iter().collect(),
            title_by_id: [(1, "Inbox".to_string()), (2, "Sent".to_string())]
                .into_iter()
                .collect(),
        })
    });
    crate::system::window::window_title::fill_from_probes(&mut records, &probes);

    let windows = assemble_global_windows(records, &owners, false, |_| {
        Ok(ax_state(Some((Some("Sent".into()), None))))
    })
    .unwrap();

    assert_eq!(windows[0].title, "Inbox");
    assert!(!windows[0].state.is_focused);
    assert_eq!(windows[1].title, "Sent");
    assert!(windows[1].state.is_focused);
}
