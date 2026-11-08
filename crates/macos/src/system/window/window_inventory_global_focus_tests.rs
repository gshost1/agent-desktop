use super::*;

pub(super) fn assemble_global_windows(
    records: Vec<WindowRecord>,
    owners: &Owners,
    focused_only: bool,
    read_frontmost: impl FnMut(i32) -> Result<WindowAxState, AdapterError>,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let capture = capture_frontmost_focus(records, owners, read_frontmost)?;
    finish_global_windows(capture, &FxHashMap::default(), focused_only)
}

fn untitled(pid: i32, window_number: i64) -> WindowRecord {
    let mut record = record(pid, window_number, "");
    record.title = None;
    record
}

fn walked(ids: &[i64], titles: &[(i64, &str)]) -> WindowAxState {
    let mut state = ax_state(None);
    state.accessible_window_ids = ids.iter().copied().collect();
    state.title_by_id = titles
        .iter()
        .map(|(id, title)| (*id, (*title).to_owned()))
        .collect();
    state
}

fn capture_once(
    records: Vec<WindowRecord>,
    focused_only: bool,
    focused: Option<(Option<String>, Option<i64>)>,
    probe: impl FnOnce(&[WindowRecord], &[i32]) -> FxHashMap<i32, WindowAxState>,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let owners = owners(&[(10, 100.0), (20, 200.0)], Some(10));
    capture_global_with(
        focused_only,
        || Ok(owners.clone()),
        |_| Ok(records.clone()),
        |_| Ok(ax_state(focused.clone())),
        probe,
    )
}

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
fn focused_only_without_a_window_number_joins_focus_on_the_probed_title() {
    let mut probed = Vec::new();

    let windows = capture_once(
        vec![untitled(10, 1), untitled(10, 2), untitled(20, 3)],
        true,
        Some((Some("Inbox".into()), None)),
        |_, pids| {
            probed.extend_from_slice(pids);
            FxHashMap::from_iter([(10, walked(&[1, 2], &[(1, "Sent"), (2, "Inbox")]))])
        },
    )
    .unwrap();

    assert_eq!(probed, vec![10]);
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].id, "w-2");
    assert_eq!(windows[0].title, "Inbox");
    assert!(windows[0].state.is_focused);
}

#[test]
fn untitled_records_take_probed_titles_before_the_focus_join() {
    let windows = capture_once(
        vec![untitled(10, 1), untitled(10, 2), untitled(20, 3)],
        false,
        Some((Some("Sent".into()), None)),
        |_, _| FxHashMap::from_iter([(10, walked(&[1, 2], &[(1, "Inbox"), (2, "Sent")]))]),
    )
    .unwrap();

    assert_eq!(windows[0].title, "Inbox");
    assert!(!windows[0].state.is_focused);
    assert_eq!(windows[1].title, "Sent");
    assert!(windows[1].state.is_focused);
    assert_eq!(windows[2].title, "App-20");
}

#[test]
fn failed_owner_probe_keeps_unknown_windows_selectable() {
    let windows = capture_once(
        vec![
            record(10, 1, "Front"),
            record(10, 2, "Hidden"),
            record(20, 3, "Back"),
        ],
        false,
        Some((None, Some(1))),
        |_, pids| {
            assert_eq!(pids, [10, 20]);
            FxHashMap::from_iter([(10, walked(&[1], &[]))])
        },
    )
    .unwrap();

    let accessible = windows
        .iter()
        .map(|window| window.state.accessible)
        .collect::<Vec<_>>();
    assert_eq!(accessible, [true, false, true]);
}

#[test]
fn global_capture_probes_owners_once_after_the_owner_snapshots_agree() {
    let owners = owners(&[(10, 100.0), (20, 200.0)], Some(10));
    let events = std::cell::RefCell::new(Vec::new());
    let snapshot_count = std::cell::Cell::new(0);

    let windows = capture_global_with(
        false,
        || {
            let ordinal = snapshot_count.get();
            events
                .borrow_mut()
                .push(if ordinal == 0 { "a" } else { "b" });
            snapshot_count.set(ordinal + 1);
            Ok(owners.clone())
        },
        |eligible| {
            events.borrow_mut().push("cg");
            assert_eq!(eligible, &FxHashSet::from_iter([10, 20]));
            Ok(vec![record(10, 1, "Front")])
        },
        |_| {
            events.borrow_mut().push("ax");
            Ok(ax_state(Some((Some("Front".into()), Some(1)))))
        },
        |_, _| {
            events.borrow_mut().push("probe");
            FxHashMap::default()
        },
    )
    .unwrap();

    assert_eq!(&*events.borrow(), &["a", "cg", "ax", "b", "probe"]);
    assert_eq!(windows.len(), 1);
}

#[test]
fn owner_churn_retries_without_probing_and_wins_over_stale_ax_failure() {
    let snapshots = std::cell::RefCell::new(std::collections::VecDeque::from([
        owners(&[(10, 100.0)], Some(10)),
        owners(&[(10, 106.0)], Some(10)),
    ]));
    let snapshot_count = std::cell::Cell::new(0);
    let probes = std::cell::Cell::new(0);

    let error = capture_global_with(
        false,
        || {
            snapshot_count.set(snapshot_count.get() + 1);
            Ok(snapshots.borrow_mut().pop_front().unwrap())
        },
        |_| Ok(vec![record(10, 1, "Front")]),
        |_| Err(AdapterError::permission_denied()),
        |_, _| {
            probes.set(probes.get() + 1);
            FxHashMap::default()
        },
    )
    .unwrap_err();

    assert_eq!(snapshot_count.get(), 2);
    assert_eq!(probes.get(), 0);
    assert_eq!(error.details.unwrap()["phase"], "appkit_snapshot_changed");
}
