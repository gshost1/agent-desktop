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
fn a_running_child_absent_from_a_read_registry_settles_after_the_not_on_bus_grace() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::ZERO);
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let owner = poll_owner(
        &mut spawned,
        &query,
        stop_after_reads(&reads, 3, Some(vec![app(9, "other")])),
        |_| Step::Wait,
        || OffBus::Windowed,
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
            |_| Step::Wait,
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
        |_| Step::Wait,
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
        |_| Step::Wait,
        || OffBus::Windowed,
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
        |_| Step::Wait,
        || {
            observed.set(observed.get() + 1);
            OffBus::Starting
        },
    );
    assert_eq!(owner.unwrap(), None);
    assert!(started.elapsed() >= std::time::Duration::from_millis(400));
    assert!(observed.get() > 1, "observed {} times", observed.get());
    assert_eq!(query.off_bus.get(), OffBus::Starting);
}

#[test]
fn a_window_or_a_bridge_turned_off_ends_the_wait_after_the_grace() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    for seen in [OffBus::Windowed, OffBus::BridgeOff] {
        let query = query_until(&identity, &before, 30_000);
        let reads = std::cell::Cell::new(0);
        let mut spawned = running_child(4242);
        let owner = poll_owner(
            &mut spawned,
            &query,
            stop_after_reads(&reads, 3, Some(Vec::new())),
            |_| Step::Wait,
            || seen,
        );
        assert_eq!(owner.unwrap(), None);
        assert_eq!(reads.get(), 1);
        assert_eq!(query.off_bus.get(), seen);
    }
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
        |_| Step::Wait,
        || panic!("the process is examined only after the grace"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Internal);
}
