use std::time::Duration;

use super::*;
use crate::wait::deadline::VirtualClock;

fn scratch_lock(name: &str) -> (crate::refs::refs_test_support::TempDir, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-file-lock-{name}-{}-{}",
        std::process::id(),
        crate::refs::new_snapshot_id()
    ));
    crate::storage::private_file_parent::ensure_private(&dir).expect("private scratch directory");
    let path = dir.join("state.lock");
    (crate::refs::refs_test_support::TempDir::adopt(dir), path)
}

fn acquire_error(path: &Path, deadline: Deadline) -> AdapterError {
    match FileLock::acquire(path, deadline, "test lock") {
        Ok(_) => panic!("the lock must not be handed out"),
        Err(error) => error,
    }
}

#[test]
fn a_free_lock_under_a_spent_deadline_reports_a_deadline_expiry() {
    let (_dir, path) = scratch_lock("spent-free");
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(10).unwrap();
    clock.advance(Duration::from_millis(15));

    let error = acquire_error(&path, deadline);

    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "deadline");
    assert_eq!(details["timeout_ms"], 10);
    assert_eq!(details["elapsed_ms"], 15);
    assert!(details.get("contention_count").is_none());
    assert!(!FileLock::is_held(&path));
}

#[test]
fn a_wait_on_a_held_lock_reports_lock_contention() {
    let (_dir, path) = scratch_lock("held-wait");
    let clock = VirtualClock::freeze();
    let holder = FileLock::acquire(&path, Deadline::after(1_000).unwrap(), "holder").unwrap();
    let deadline = Deadline::after(10).unwrap();

    let error = acquire_error(&path, deadline);

    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "lock_timeout");
    assert_eq!(details["purpose"], "test lock");
    assert_eq!(details["path"], "state.lock");
    assert!(details["contention_count"].as_u64().unwrap() >= 1);
    assert_eq!(clock.elapsed(), Duration::from_millis(10));
    drop(holder);
}

#[test]
fn an_expiry_after_contention_keeps_the_lock_timeout_label() {
    let clock = VirtualClock::freeze();
    let deadline = Deadline::after(10).unwrap();
    clock.advance(Duration::from_millis(10));
    let path = Path::new("/state/held.lock");

    let contended = expiry_error(deadline, "test lock", path, 3)
        .details
        .expect("details");
    let uncontended = expiry_error(deadline, "test lock", path, 0)
        .details
        .expect("details");

    assert_eq!(contended["kind"], "lock_timeout");
    assert_eq!(contended["contention_count"], 3);
    assert_eq!(contended["path"], "held.lock");
    assert_eq!(uncontended["kind"], "deadline");
    assert!(uncontended.get("contention_count").is_none());
}
