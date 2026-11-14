use super::{ScopedRead, owner_still_running, validate_scoped_owners};
use crate::system::window::window_inventory_global::{OwnerGeneration, OwnerSnapshotView};
use agent_desktop_core::AdapterError;
use rustc_hash::FxHashMap;

#[derive(Clone)]
struct ScopedOwners(FxHashMap<i32, f64>);

impl OwnerSnapshotView for ScopedOwners {
    fn eligible_pids(&self) -> rustc_hash::FxHashSet<i32> {
        self.0.keys().copied().collect()
    }

    fn generation(
        &self,
        pid: i32,
    ) -> Option<crate::system::window::window_inventory_global::OwnerGeneration> {
        self.0
            .get(&pid)
            .copied()
            .map(crate::system::window::window_inventory_global::OwnerGeneration::LaunchTime)
    }

    fn frontmost_pid(&self) -> Option<i32> {
        None
    }

    fn same_generation(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

type Recaptured = Result<(ScopedOwners, rustc_hash::FxHashSet<i32>), AdapterError>;

fn validate(
    owners: &ScopedOwners,
    selected: &rustc_hash::FxHashSet<i32>,
    still_running: impl FnMut(i32, OwnerGeneration) -> Result<bool, AdapterError>,
    recapture: impl FnOnce() -> Recaptured,
) -> Result<(), AdapterError> {
    validate_scoped_owners(
        ScopedRead {
            owners,
            selected,
            records: &[],
        },
        still_running,
        recapture,
    )
}

fn no_second_snapshot() -> Recaptured {
    panic!("libproc answered, so no second AppKit snapshot is taken")
}

#[test]
fn scoped_validation_checks_only_the_selected_owners() {
    let owners = ScopedOwners(FxHashMap::from_iter([(10, 100.0), (20, 200.0)]));
    let selected = rustc_hash::FxHashSet::from_iter([10]);
    let mut checked = Vec::new();
    let still_running = |pid, _| {
        checked.push(pid);
        Ok(true)
    };
    validate(&owners, &selected, still_running, no_second_snapshot).unwrap();
    assert_eq!(checked, vec![10]);
}

#[test]
fn scoped_validation_rejects_a_selected_owner_that_exited_or_relaunched() {
    let owners = ScopedOwners(FxHashMap::from_iter([(10, 100.0), (11, 110.0)]));
    let selected = rustc_hash::FxHashSet::from_iter([10, 11]);
    let still_running = |pid, generation| {
        assert!(matches!(generation, OwnerGeneration::LaunchTime(_)));
        Ok(pid != 11)
    };
    let error = validate(&owners, &selected, still_running, no_second_snapshot).unwrap_err();
    assert!(error.is_explicitly_retryable());
    let details = error.details.unwrap();
    assert_eq!(details["phase"], "selected_owner_changed");
    assert_eq!(details["pid"], 11);
}

#[test]
fn scoped_validation_rejects_a_selected_pid_the_snapshot_never_named() {
    let owners = ScopedOwners(FxHashMap::from_iter([(10, 100.0)]));
    let selected = rustc_hash::FxHashSet::from_iter([12]);
    let error = validate(&owners, &selected, |_, _| Ok(true), no_second_snapshot).unwrap_err();
    assert_eq!(error.details.unwrap()["phase"], "selected_owner_changed");
}

fn unreadable(_: i32, _: OwnerGeneration) -> Result<bool, AdapterError> {
    Err(AdapterError::new(
        agent_desktop_core::ErrorCode::PermDenied,
        "denied",
    ))
}

#[test]
fn an_owner_libproc_cannot_read_is_confirmed_by_a_second_snapshot() {
    let owners = ScopedOwners(FxHashMap::from_iter([(10, 100.0), (20, 200.0)]));
    let selected = rustc_hash::FxHashSet::from_iter([10]);
    let after = ScopedOwners(FxHashMap::from_iter([(10, 100.0), (30, 300.0)]));
    validate(&owners, &selected, unreadable, || {
        Ok((after, selected.clone()))
    })
    .unwrap();
}

#[test]
fn a_second_snapshot_that_replaced_an_unreadable_owner_rejects_the_read() {
    let owners = ScopedOwners(FxHashMap::from_iter([(10, 100.0)]));
    let selected = rustc_hash::FxHashSet::from_iter([10]);
    let replaced = ScopedOwners(FxHashMap::from_iter([(10, 101.0)]));
    let error = validate(&owners, &selected, unreadable, || {
        Ok((replaced, selected.clone()))
    })
    .unwrap_err();
    assert_eq!(
        error.details.unwrap()["phase"],
        "selected_appkit_snapshot_changed"
    );
    let moved = ScopedOwners(FxHashMap::from_iter([(11, 101.0)]));
    let moved_pids = rustc_hash::FxHashSet::from_iter([11]);
    let error = validate(&owners, &selected, unreadable, || Ok((moved, moved_pids))).unwrap_err();
    assert_eq!(
        error.details.unwrap()["phase"],
        "selected_appkit_snapshot_changed"
    );
}

#[test]
fn a_running_owner_matches_its_own_launch_time_and_not_another() {
    let pid = i32::try_from(std::process::id()).unwrap();
    let launch_time = crate::system::process::process_identity::ProcessIdentity::capture(pid)
        .unwrap()
        .unwrap()
        .launch_time_seconds();
    assert!(owner_still_running(pid, OwnerGeneration::LaunchTime(launch_time)).unwrap());
    assert!(!owner_still_running(pid, OwnerGeneration::LaunchTime(launch_time - 3_600.0)).unwrap());
    assert!(owner_still_running(pid, OwnerGeneration::LiveProcess).unwrap());
}

#[test]
fn an_exited_owner_is_not_running_in_either_generation() {
    let mut child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
    let pid = i32::try_from(child.id()).unwrap();
    child.wait().unwrap();
    assert!(!owner_still_running(pid, OwnerGeneration::LiveProcess).unwrap());
    assert!(!owner_still_running(pid, OwnerGeneration::LaunchTime(1.0)).unwrap());
}

#[test]
fn an_exited_owner_not_yet_reaped_is_not_running() {
    let mut child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
    let pid = i32::try_from(child.id()).unwrap();
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    let waited = unsafe {
        libc::waitid(
            libc::P_PID,
            child.id(),
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOWAIT,
        )
    };
    assert_eq!(waited, 0);
    assert!(!owner_still_running(pid, OwnerGeneration::LiveProcess).unwrap());
    assert!(child.try_wait().unwrap().is_some());
}
