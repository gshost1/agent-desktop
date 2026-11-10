use std::cell::Cell;

use super::*;
use agent_desktop_core::WindowInfo;
use agent_desktop_core::system::launch_presentation::LaunchPresentation;

fn long_deadline() -> Deadline {
    Deadline::after(30_000).expect("a deadline")
}

fn activating() -> LaunchOptions {
    LaunchOptions {
        timeout_ms: 30_000,
        presentation: LaunchPresentation {
            activate: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn window(id: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: id.into(),
        app: "probe".into(),
        pid: agent_desktop_core::ProcessId::new(7),
        process_instance: Some("instance".into()),
        bounds: None,
        state: Default::default(),
    }
}

fn absent_until<'a>(
    reads: &'a Cell<u32>,
    limit: u32,
) -> impl FnMut() -> Result<LaunchWindow, AdapterError> + 'a {
    move || {
        reads.set(reads.get() + 1);
        if reads.get() > limit {
            return Err(AdapterError::internal("the launch wait kept polling"));
        }
        Ok(LaunchWindow::Absent)
    }
}

fn gone_after<'a>(
    polls: &'a Cell<u32>,
    alive_polls: u32,
) -> impl FnMut() -> Result<(), AdapterError> + 'a {
    move || {
        polls.set(polls.get() + 1);
        if polls.get() > alive_polls {
            Err(launch_target_gone(7))
        } else {
            Ok(())
        }
    }
}

#[test]
fn an_application_that_exits_after_finishing_startup_ends_an_activating_wait() {
    let reads = Cell::new(0);
    let polls = Cell::new(0);
    let error = settle_window(
        SettleProbes {
            read: absent_until(&reads, 5),
            startup_finished: || Ok(true),
            alive: gone_after(&polls, 2),
        },
        &activating(),
        long_deadline(),
    )
    .expect_err("an exited application is not a launch");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(error.message.contains("exited"), "{}", error.message);
    assert_eq!(reads.get(), 3);
}

#[test]
fn an_application_that_exits_inside_the_startup_grace_is_not_reported_as_launched() {
    let reads = Cell::new(0);
    let polls = Cell::new(0);
    let error = settle_window(
        SettleProbes {
            read: absent_until(&reads, 5),
            startup_finished: || Ok(true),
            alive: gone_after(&polls, 1),
        },
        &LaunchOptions {
            timeout_ms: 30_000,
            ..Default::default()
        },
        long_deadline(),
    )
    .expect_err("an exit inside the grace is not a launch");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(reads.get(), 2);
}

#[test]
fn several_windows_keep_the_wait_going_and_end_it_without_claiming_one() {
    let reads = Cell::new(0);
    let found = settle_window(
        SettleProbes {
            read: || {
                reads.set(reads.get() + 1);
                Ok(LaunchWindow::Several(vec!["w-1".into(), "w-2".into()]))
            },
            startup_finished: || Ok(true),
            alive: || Ok(()),
        },
        &LaunchOptions {
            timeout_ms: 30_000,
            ..Default::default()
        },
        long_deadline(),
    )
    .expect("several windows are a launch");
    assert!(reads.get() > 1, "the wait ended on the first read");
    let (window, note) = found.into_parts();
    assert!(window.is_none());
    let note = note.expect("a note");
    assert!(note.contains("w-1, w-2"), "{note}");
}

#[test]
fn a_zero_timeout_reads_once() {
    let reads = Cell::new(0);
    let found = settle_window(
        SettleProbes {
            read: absent_until(&reads, 5),
            startup_finished: || Ok(false),
            alive: || Ok(()),
        },
        &LaunchOptions {
            timeout_ms: 0,
            ..Default::default()
        },
        long_deadline(),
    )
    .expect("no window is a launch outcome");
    assert!(matches!(found, LaunchWindow::Absent));
    assert_eq!(reads.get(), 1);
}

#[test]
fn focus_that_settles_after_several_windows_claims_the_focused_window() {
    let reads = Cell::new(0);
    let found = settle_window(
        SettleProbes {
            read: || {
                reads.set(reads.get() + 1);
                if reads.get() == 1 {
                    Ok(LaunchWindow::Several(vec!["w-1".into(), "w-2".into()]))
                } else {
                    Ok(LaunchWindow::Shown(window("w-2")))
                }
            },
            startup_finished: || Ok(false),
            alive: || Ok(()),
        },
        &activating(),
        long_deadline(),
    )
    .expect("a window");
    assert!(matches!(found, LaunchWindow::Shown(shown) if shown.id == "w-2"));
}

#[test]
fn a_window_read_first_is_the_launch_window() {
    let found = settle_window(
        SettleProbes {
            read: || Ok(LaunchWindow::Shown(window("w-1"))),
            startup_finished: || Ok(false),
            alive: || Err(launch_target_gone(7)),
        },
        &activating(),
        long_deadline(),
    )
    .expect("a window");
    assert!(matches!(found, LaunchWindow::Shown(shown) if shown.id == "w-1"));
}

#[test]
fn waiting_ends_only_after_a_completed_startup_plus_its_grace() {
    let now = std::time::Instant::now();

    assert!(!grace_over(None, now));
    assert!(!grace_over(now.checked_add(STARTUP_GRACE), now));
    assert!(grace_over(Some(now), now));
}

#[test]
fn app_unresponsive_and_timeout_window_reads_are_not_ready_yet() {
    assert!(window_read_is_not_ready_yet(
        &AdapterError::app_unresponsive("Numbers")
    ));
    assert!(window_read_is_not_ready_yet(&AdapterError::timeout("busy")));
}

#[test]
fn every_other_error_code_ends_the_wait_immediately() {
    assert!(!window_read_is_not_ready_yet(&AdapterError::stale_ref(
        "@e1"
    )));
    assert!(!window_read_is_not_ready_yet(
        &AdapterError::permission_denied()
    ));
}
