use super::*;
use agent_desktop_core::{WindowInfo, WindowState};
use std::cell::Cell;

#[test]
fn timeout_then_success_labels_only_bridged_windows_accessible() {
    let mut windows = vec![window(1), window(2)];
    let mut probes = Vec::new();
    let start = Instant::now();
    label_until(
        &mut windows,
        start + Duration::from_secs(2),
        |pid, wanted, probe| {
            assert_eq!(pid, 10);
            assert_eq!(wanted, &FxHashSet::from_iter([1, 2]));
            probes.push(probe);
            if probes.len() == 1 {
                Err(AdapterError::timeout("slow initial read"))
            } else {
                let mut titles = WindowAxTitles::default();
                titles.record(1, Some("Document".into()));
                Ok(titles)
            }
        },
        || start,
    );
    assert_eq!(
        probes,
        vec![start + LABEL_PROBE, start + LABEL_RETRY_BUDGET]
    );
    assert!(windows[0].state.accessible);
    assert_eq!(windows[0].title, "Document");
    assert!(!windows[1].state.accessible);
    assert!(windows[1].title.is_empty());
}

#[test]
fn shared_retry_budget_bounds_hung_apps_and_preserves_unretried_labels() {
    let run = || {
        let start = Instant::now();
        let clock = Cell::new(start);
        let mut windows = (1..=12)
            .rev()
            .map(|id| {
                let mut window = window(id);
                window.pid = (id as u32).into();
                window.state.accessible = id % 2 == 0;
                window
            })
            .collect::<Vec<_>>();
        let mut attempts = Vec::new();
        label_until(
            &mut windows,
            start + Duration::from_secs(5),
            |pid, _, probe| {
                attempts.push(pid);
                clock.set(probe);
                Err(AdapterError::timeout("hung app"))
            },
            || clock.get(),
        );
        assert_eq!(clock.get() - start, LABEL_PROBE * 12 + LABEL_RETRY_BUDGET);
        assert_eq!(attempts, (1..=12).chain([1]).collect::<Vec<_>>());
        let labels = windows
            .iter()
            .map(|window| window.state.accessible)
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            (1..=12).rev().map(|id| id % 2 == 0).collect::<Vec<_>>()
        );
        labels
    };
    assert_eq!(run(), run());
}

#[test]
fn non_timeout_error_and_successful_empty_read_are_not_retried() {
    for permission_error in [false, true] {
        let mut windows = vec![window(1)];
        let mut attempts = 0;
        let start = Instant::now();
        label_until(
            &mut windows,
            start + Duration::from_secs(2),
            |_, _, _| {
                attempts += 1;
                if permission_error {
                    Err(AdapterError::permission_denied())
                } else {
                    Ok(WindowAxTitles::default())
                }
            },
            || start,
        );
        assert_eq!(attempts, 1);
        assert_eq!(windows[0].state.accessible, permission_error);
    }
}

#[test]
fn overall_deadline_without_retry_margin_preserves_first_read() {
    for remaining in [Duration::ZERO, LABEL_PROBE * 2] {
        let start = Instant::now();
        let deadline = start + remaining;
        let mut windows = vec![window(1)];
        windows[0].state.accessible = false;
        let mut attempts = 0;
        label_until(
            &mut windows,
            deadline,
            |_, _, probe| {
                attempts += 1;
                assert_eq!(probe, (start + LABEL_PROBE).min(deadline));
                Err(AdapterError::timeout("overall deadline exhausted"))
            },
            || start,
        );
        assert_eq!(attempts, 1);
        assert!(!windows[0].state.accessible);
    }
}

#[test]
fn empty_inventory_does_not_read() {
    let start = Instant::now();
    label_until(
        &mut [],
        start,
        |_, _, _| panic!("unexpected read"),
        || start,
    );
}

fn window(number: i64) -> WindowInfo {
    WindowInfo {
        id: format!("w-{number}"),
        title: String::new(),
        app: "Application".into(),
        pid: 10.into(),
        process_instance: None,
        bounds: None,
        state: WindowState {
            is_focused: false,
            accessible: true,
            minimized: None,
            visible: Some(false),
        },
    }
}
