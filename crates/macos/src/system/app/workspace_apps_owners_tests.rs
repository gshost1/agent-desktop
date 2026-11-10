use super::*;

fn deadline() -> Instant {
    Instant::now() + std::time::Duration::from_secs(1)
}

fn payload(selected_only: bool) -> Vec<u8> {
    let mut snapshot = serde_json::json!({
        "applications": [
            {"name":"Mail","pid":10,"launch_time":100.25,"activation_policy":"regular"}
        ],
        "frontmost_pid": 11,
        "frontmost_launch_time": 101.5
    });
    if selected_only {
        snapshot["selected_only"] = serde_json::Value::Bool(true);
    }
    serde_json::to_vec(&snapshot).unwrap()
}

#[test]
fn a_named_read_without_the_frontmost_record_reports_no_frontmost_app() {
    let snapshot = window_owner_snapshot_from_json(&payload(true), deadline()).unwrap();

    assert_eq!(snapshot.frontmost_pid, None);
    assert_eq!(snapshot.eligible_pids().len(), 1);
    assert!(snapshot.owner(10).is_some());
}

#[test]
fn a_full_read_without_the_frontmost_record_still_fails() {
    let error = window_owner_snapshot_from_json(&payload(false), deadline()).unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::AppUnresponsive);
}

fn sorted(mut pids: Vec<i32>) -> Vec<i32> {
    pids.sort_unstable();
    pids
}

fn full_read_names() -> Vec<String> {
    let snapshot = window_owner_snapshot_until(deadline()).unwrap();
    let mut names: Vec<String> = snapshot.owners.iter().map(|o| o.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn a_named_read_finds_the_same_apps_as_a_full_read() {
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded().unwrap();
    let Some(target) = full_read_names().first().cloned() else {
        return;
    };
    for _ in 0..8 {
        let before = window_owner_snapshot_until(deadline()).unwrap();
        let named = named_window_owner_snapshot_until(&target, deadline()).unwrap();
        let after = window_owner_snapshot_until(deadline()).unwrap();
        if before != after {
            continue;
        }
        assert_eq!(
            sorted(named.matching_pids(&target).into_iter().collect()),
            sorted(before.matching_pids(&target).into_iter().collect())
        );
        assert!(named.owners.iter().all(|owner| owner.name == target));
        return;
    }
    panic!("the running-app list changed during every attempt");
}

#[test]
fn a_named_read_for_a_name_no_app_has_holds_every_owner() {
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded().unwrap();
    let missing = "agent-desktop no app has this name";
    for _ in 0..8 {
        let before = window_owner_snapshot_until(deadline()).unwrap();
        let named = named_window_owner_snapshot_until(missing, deadline()).unwrap();
        let after = window_owner_snapshot_until(deadline()).unwrap();
        if before != after {
            continue;
        }
        assert_eq!(named, before);
        return;
    }
    panic!("the running-app list changed during every attempt");
}
