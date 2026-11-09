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
fn an_untitled_window_never_joins_an_untitled_focused_identity() {
    let owners = owners(&[(10, 100.0)], Some(10));
    let mut untitled = record(10, 1, "unused");
    untitled.title = None;

    let joined = assemble_global_windows(vec![untitled], &owners, false, |_| {
        Ok(ax_state(Some((None, None))))
    });

    let reported_focus = match joined {
        Ok(windows) => windows.iter().any(|window| window.state.is_focused),
        Err(_) => false,
    };
    assert!(!reported_focus);
}
