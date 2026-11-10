use std::cell::Cell;

use super::*;
use agent_desktop_core::DeliveryDisposition;

fn window_of(pid: u32) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "probe".into(),
        app: "probe".into(),
        pid: ProcessId::new(pid),
        process_instance: Some("instance".into()),
        bounds: None,
        state: Default::default(),
    }
}

fn long_deadline() -> Deadline {
    Deadline::after(30_000).expect("a deadline")
}

fn reads_until(
    reads: &Cell<u32>,
    limit: u32,
    window: Option<WindowInfo>,
) -> impl FnMut() -> Result<Option<WindowInfo>, AdapterError> + '_ {
    move || {
        reads.set(reads.get() + 1);
        if reads.get() > limit {
            return Err(AdapterError::internal("the window wait kept polling"));
        }
        Ok(window.clone())
    }
}

#[test]
fn a_launched_process_that_exits_without_a_window_ends_the_wait_on_that_poll() {
    let reads = Cell::new(0);
    let exits = Cell::new(0);
    let error = watch_window(
        reads_until(&reads, 3, None),
        || {
            exits.set(exits.get() + 1);
            (exits.get() >= 2).then(|| exited_before_window("stub.exe", ProcessId::new(7), 0))
        },
        true,
        long_deadline(),
    )
    .expect_err("an exited launcher is not a launch");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(reads.get(), 2);
    assert_eq!(error.details.expect("details")["kind"], "exited");
}

#[test]
fn a_window_seen_before_the_exit_check_is_the_launch_window() {
    let reads = Cell::new(0);
    let window = watch_window(
        reads_until(&reads, 3, Some(window_of(7))),
        || panic!("the exit check runs only when no window was read"),
        true,
        long_deadline(),
    )
    .expect("a window");
    assert_eq!(window.map(|window| window.pid), Some(ProcessId::new(7)));
}

#[test]
fn a_single_read_for_a_running_process_reports_no_window() {
    let reads = Cell::new(0);
    let window = watch_window(
        reads_until(&reads, 3, None),
        || None,
        false,
        long_deadline(),
    )
    .expect("no window is a launch outcome");
    assert!(window.is_none());
    assert_eq!(reads.get(), 1);
}

#[test]
fn a_running_process_without_a_window_is_polled_until_the_deadline() {
    let reads = Cell::new(0);
    let error = watch_window(reads_until(&reads, 3, None), || None, true, long_deadline())
        .expect_err("the fake read ends the poll");
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(reads.get(), 4);
}

#[test]
fn an_exit_is_reported_as_delivered_with_its_status_and_kind() {
    let exited = exited_before_window("stub.exe", ProcessId::new(7), 3);
    assert_eq!(exited.code, ErrorCode::ActionFailed);
    assert_eq!(
        exited.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    let details = exited.details.expect("details");
    assert_eq!(details["kind"], "exited");
    assert_eq!(details["status"], 3);
    assert_eq!(details["pid"], 7);
    assert!(exited.message.contains("exited with status 3"));
    let crashed = exited_before_window("stub.exe", ProcessId::new(7), 0xC000_0005);
    assert_eq!(crashed.details.expect("details")["kind"], "crashed");
    assert!(
        crashed.message.contains("0xc0000005"),
        "{}",
        crashed.message
    );
}
