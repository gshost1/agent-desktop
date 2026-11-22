use super::tests::{app, keys};
use super::*;

fn running_child(pid: u32) -> Spawned {
    Spawned {
        child: None,
        pid,
        identifier: "plain-x11-tool".to_owned(),
    }
}

fn empty_baseline() -> super::super::baseline::Baseline {
    super::super::baseline::Baseline {
        processes: HashSet::new(),
        residents: Vec::new(),
        instances: 0,
    }
}

fn query_with_grace<'a>(
    identity: &'a Option<crate::system::process::executable_identity::ExecutableIdentity>,
    before: &'a super::super::baseline::Baseline,
    grace: std::time::Duration,
) -> OwnerQuery<'a> {
    let deadline =
        agent_desktop_core::Deadline::from_duration(std::time::Duration::from_secs(30)).unwrap();
    OwnerQuery::with_grace(identity, before, deadline, Some(grace))
        .settling_after(std::time::Duration::ZERO)
}

fn stop_after_reads(
    reads: &std::cell::Cell<u32>,
    limit: u32,
    listed: Option<Vec<Running>>,
) -> impl FnMut() -> Result<Option<Vec<Running>>, AdapterError> + '_ {
    move || {
        reads.set(reads.get() + 1);
        if reads.get() > limit {
            return Err(AdapterError::internal("the owner search kept polling"));
        }
        Ok(listed.clone())
    }
}

#[test]
fn a_bridge_turned_off_ends_the_wait_after_the_grace() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::ZERO);
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let owner = poll_owner(
        &mut spawned,
        &query,
        stop_after_reads(&reads, 3, Some(vec![app(9, "other")])),
        |_, _| Step::Wait,
        || OffBus::BridgeOff,
    );
    assert_eq!(owner.unwrap(), None);
    assert_eq!(reads.get(), 1);
}

#[test]
fn the_not_on_bus_grace_waits_while_it_runs_and_ignores_unread_registries() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    for (grace, listed) in [
        (std::time::Duration::from_secs(30), Some(Vec::new())),
        (std::time::Duration::ZERO, None),
    ] {
        let query = query_with_grace(&identity, &before, grace);
        let reads = std::cell::Cell::new(0);
        let mut spawned = running_child(4242);
        let error = poll_owner(
            &mut spawned,
            &query,
            stop_after_reads(&reads, 3, listed),
            |_, _| Step::Wait,
            || OffBus::Windowed,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(reads.get(), 4);
    }
}

#[test]
fn an_owner_found_inside_the_grace_is_the_launch_owner() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::ZERO);
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let own = app(4242, "plain-x11-tool");
    let owner = poll_owner(
        &mut spawned,
        &query,
        stop_after_reads(&reads, 3, Some(vec![own.clone()])),
        |_, _| Step::Wait,
        || OffBus::Windowed,
    );
    assert_eq!(owner.unwrap(), Some(own));
}

#[test]
fn an_activating_launch_waits_for_the_bus_until_its_deadline() {
    let identity = None;
    let before = empty_baseline();
    let deadline =
        agent_desktop_core::Deadline::from_duration(std::time::Duration::from_secs(60)).unwrap();
    let plain = OwnerQuery::new(&identity, &before, deadline, false);
    let activating = OwnerQuery::new(&identity, &before, deadline, true);
    assert!(plain.not_on_bus.remaining() <= std::time::Duration::from_secs(5));
    assert!(plain.not_on_bus.remaining() > std::time::Duration::from_secs(4));
    assert!(activating.not_on_bus.remaining() > std::time::Duration::from_secs(59));
    let short =
        agent_desktop_core::Deadline::from_duration(std::time::Duration::from_millis(500)).unwrap();
    let bounded = OwnerQuery::new(&identity, &before, short, false);
    assert!(bounded.not_on_bus.remaining() <= std::time::Duration::from_millis(500));
}

#[test]
fn a_grace_that_expires_during_a_read_is_settled_only_by_the_next_read() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::from_millis(1_000));
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let owner = poll_owner(
        &mut spawned,
        &query,
        || {
            reads.set(reads.get() + 1);
            if reads.get() == 1 {
                std::thread::sleep(std::time::Duration::from_millis(1_500));
            }
            Ok(Some(Vec::new()))
        },
        |_, _| Step::Wait,
        || OffBus::BridgeOff,
    );
    assert_eq!(owner.unwrap(), None);
    assert_eq!(reads.get(), 2);
}

fn query_until<'a>(
    identity: &'a Option<crate::system::process::executable_identity::ExecutableIdentity>,
    before: &'a super::super::baseline::Baseline,
    deadline_ms: u64,
) -> OwnerQuery<'a> {
    let deadline =
        agent_desktop_core::Deadline::from_duration(std::time::Duration::from_millis(deadline_ms))
            .unwrap();
    OwnerQuery::with_grace(identity, before, deadline, Some(std::time::Duration::ZERO))
}

#[test]
fn a_process_that_may_still_be_starting_is_waited_for_until_the_deadline() {
    let identity = keys(&["slow-starting-app"]);
    let before = empty_baseline();
    let query = query_until(&identity, &before, 400);
    let observed = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let started = std::time::Instant::now();
    let owner = poll_owner(
        &mut spawned,
        &query,
        || Ok(Some(Vec::new())),
        |_, _| Step::Wait,
        || {
            observed.set(observed.get() + 1);
            OffBus::Starting
        },
    );
    assert_eq!(owner.unwrap(), None);
    assert!(started.elapsed() >= std::time::Duration::from_millis(400));
    assert!(observed.get() > 1, "observed {} times", observed.get());
    assert_eq!(query.seen.get(), Seen::OffBus(OffBus::Starting));
}

#[test]
fn the_process_is_not_examined_inside_the_grace() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::from_secs(30));
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let error = poll_owner(
        &mut spawned,
        &query,
        stop_after_reads(&reads, 3, Some(Vec::new())),
        |_, _| Step::Wait,
        || panic!("the process is examined only after the grace"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Internal);
}

fn exited_launcher() -> Spawned {
    let Ok(mut child) = std::process::Command::new("/bin/true").spawn() else {
        panic!("/bin/true starts");
    };
    let _ = child.wait();
    Spawned::new(child, "forwarding-tool")
}

#[test]
fn a_clean_launcher_exit_is_settled_only_after_the_query_settle() {
    let identity = keys(&["forwarding-tool"]);
    let before = empty_baseline();
    let resident = app(77, "forwarding-tool");
    let settled = query_until(&identity, &before, 30_000).settling_after(std::time::Duration::ZERO);
    let mut spawned = exited_launcher();
    let owner = poll_owner(
        &mut spawned,
        &settled,
        || Ok(Some(Vec::new())),
        |_, settled| {
            assert!(settled, "a zero settle is settled at once");
            Step::Owner(resident.clone())
        },
        || OffBus::Starting,
    );
    assert_eq!(owner.unwrap(), Some(resident));
    let unsettled = query_until(&identity, &before, 30_000)
        .settling_after(std::time::Duration::from_secs(3_600));
    let reads = std::cell::Cell::new(0);
    let mut spawned = exited_launcher();
    let error = poll_owner(
        &mut spawned,
        &unsettled,
        stop_after_reads(&reads, 3, Some(Vec::new())),
        |_, settled| {
            assert!(!settled, "an hour-long settle has not passed");
            Step::Wait
        },
        || panic!("an exited launcher is not examined for a window"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Internal);
}

#[test]
fn a_window_off_the_bus_is_waited_for_until_an_owner_appears() {
    let identity = keys(&["late-registering-app"]);
    let before = empty_baseline();
    let query = query_until(&identity, &before, 30_000);
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let own = app(4242, "late-registering-app");
    let owner = poll_owner(
        &mut spawned,
        &query,
        || {
            reads.set(reads.get() + 1);
            Ok(Some(if reads.get() < 14 {
                Vec::new()
            } else {
                vec![own.clone()]
            }))
        },
        |_, _| Step::Wait,
        || OffBus::Windowed,
    )
    .unwrap();
    assert_eq!(owner, Some(own));
    assert_eq!(reads.get(), 14);
    assert_eq!(query.seen.get(), Seen::OffBus(OffBus::Windowed));
}

#[test]
fn a_window_off_the_bus_is_waited_for_until_the_deadline() {
    let identity = None;
    let before = empty_baseline();
    let query = query_until(&identity, &before, 2_300);
    let mut spawned = running_child(4242);
    let started = std::time::Instant::now();
    let owner = poll_owner(
        &mut spawned,
        &query,
        || Ok(Some(Vec::new())),
        |_, _| Step::Wait,
        || OffBus::Windowed,
    )
    .unwrap();
    assert_eq!(owner, None);
    assert!(started.elapsed() >= std::time::Duration::from_millis(2_300));
    assert_eq!(query.seen.get(), Seen::OffBus(OffBus::Windowed));
}
