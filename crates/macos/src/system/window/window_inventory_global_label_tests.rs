use super::*;
use agent_desktop_core::{WindowInfo, WindowState};

use crate::system::window::window_ax_state::label_accessibility;

#[test]
fn timeout_then_success_labels_only_bridged_windows_accessible() {
    let mut windows = vec![window(1), window(2)];
    let mut probes = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    label_accessibility(&mut windows, |pid, wanted| {
        read_until(pid, wanted, deadline, |pid, wanted, probe| {
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
        })
    });
    assert_eq!(probes.len(), 2);
    assert!(probes[1] > probes[0]);
    assert!(probes.iter().all(|probe| *probe <= deadline));
    assert!(windows[0].state.accessible);
    assert_eq!(windows[0].title, "Document");
    assert!(!windows[1].state.accessible);
    assert!(windows[1].title.is_empty());
}

#[test]
fn repeated_timeout_reads_exactly_twice_and_preserves_optimistic_labels() {
    let mut windows = vec![window(1), window(2)];
    let mut attempts = 0;
    let deadline = Instant::now() + Duration::from_secs(2);
    label_accessibility(&mut windows, |pid, wanted| {
        read_until(pid, wanted, deadline, |_, _, _| {
            attempts += 1;
            Err(AdapterError::timeout("unresponsive application"))
        })
    });
    assert_eq!(attempts, 2);
    assert!(windows.iter().all(|window| window.state.accessible));
}

#[test]
fn non_timeout_error_is_not_retried() {
    let mut windows = vec![window(1)];
    let mut attempts = 0;
    let deadline = Instant::now() + Duration::from_secs(2);
    label_accessibility(&mut windows, |pid, wanted| {
        read_until(pid, wanted, deadline, |_, _, _| {
            attempts += 1;
            Err(AdapterError::permission_denied())
        })
    });
    assert_eq!(attempts, 1);
    assert!(windows[0].state.accessible);
}

#[test]
fn successful_empty_read_is_not_retried_and_labels_missing_window_inaccessible() {
    let mut windows = vec![window(1)];
    let mut attempts = 0;
    let deadline = Instant::now() + Duration::from_secs(2);
    label_accessibility(&mut windows, |pid, wanted| {
        read_until(pid, wanted, deadline, |_, _, _| {
            attempts += 1;
            Ok(WindowAxTitles::default())
        })
    });
    assert_eq!(attempts, 1);
    assert!(!windows[0].state.accessible);
}

#[test]
fn both_attempts_are_capped_by_the_overall_deadline() {
    let deadline = Instant::now();
    let mut attempts = 0;
    let result = read_until(10, &FxHashSet::default(), deadline, |_, _, probe| {
        attempts += 1;
        assert_eq!(probe, deadline);
        Err(AdapterError::timeout("overall deadline exhausted"))
    });
    assert_eq!(attempts, 2);
    assert_eq!(result.unwrap_err().code, ErrorCode::Timeout);
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
