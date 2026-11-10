use super::*;

const UNUSED_PIDS: [i32; 5] = [
    i32::MAX - 1,
    i32::MAX - 2,
    i32::MAX - 3,
    i32::MAX - 4,
    i32::MAX - 5,
];

fn warmed() -> Vec<i32> {
    WARMED_APPLICATIONS
        .lock()
        .map(|warmed| warmed.clone())
        .unwrap_or_default()
}

#[test]
fn each_process_is_admitted_once_and_only_a_few_are_admitted() {
    let mut warmed = Vec::new();
    assert!(admit(&mut warmed, 10));
    assert!(!admit(&mut warmed, 10));
    assert!(!admit(&mut warmed, 0));
    assert!(!admit(&mut warmed, -1));
    for pid in 11..20 {
        admit(&mut warmed, pid);
    }
    assert_eq!(warmed, vec![10, 11, 12, 13]);
}

#[test]
fn an_unscoped_inventory_messages_no_application() {
    start_applications(&UNUSED_PIDS, Instant::now() + Duration::from_secs(1));
    let warmed = warmed();
    assert!(UNUSED_PIDS.iter().all(|pid| !warmed.contains(pid)));
}

#[test]
fn a_passed_deadline_messages_no_application() {
    start_applications(&UNUSED_PIDS[..1], Instant::now());
    assert!(!warmed().contains(&UNUSED_PIDS[0]));
}

#[test]
fn a_permission_preflight_starts_the_window_server_connection() {
    if let Ok(deadline) = agent_desktop_core::Deadline::after(5_000) {
        let _ = crate::system::permissions::preflight_report(deadline);
    }
    assert!(WINDOW_SERVER.get().is_some());
}

#[test]
fn a_warm_up_that_panics_still_counts_as_finished() {
    let before = IN_FLIGHT.lock().map(|count| *count).unwrap_or(usize::MAX);
    let (sent, received) = std::sync::mpsc::channel();
    spawn(move || {
        let _ = sent.send(());
        panic!("warm-up failed");
    });
    let _ = received.recv_timeout(Duration::from_secs(5));
    let settled_by = Instant::now() + Duration::from_secs(5);
    let mut count = usize::MAX;
    while Instant::now() < settled_by {
        count = IN_FLIGHT.lock().map(|count| *count).unwrap_or(usize::MAX);
        if count <= before {
            break;
        }
        std::thread::yield_now();
    }
    assert!(count <= before);
}
