use super::*;
use crate::mcp::call::child_command;
use crate::mcp::queued_ticket::QueuedTicket;
use agent_desktop_core::{AppError, DeliverySemantics, ErrorCode};
use serde_json::json;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

const TEST_VARIABLE: &str = "AGENT_DESKTOP_MCP_LEASE_TEST";
const WAITING: Duration = Duration::from_millis(200);
const SETTLED: Duration = Duration::from_secs(5);

fn inherited() -> LeaseInheritance {
    LeaseInheritance::Inherited(Box::new(InheritedLease::new(
        TEST_VARIABLE,
        "test-lease".into(),
    )))
}

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

fn lease_variable(lease: &ChildLease<'_>) -> Option<Option<String>> {
    let command = child_command(Path::new("agent-desktop"), &[], &words(&["click"]), lease);
    command
        .get_envs()
        .find(|(name, _)| *name == OsStr::new(TEST_VARIABLE))
        .map(|(_, value)| value.map(|value| value.to_string_lossy().into_owned()))
}

fn lent(lease: &LeaseInheritance, id: u64) -> ChildLease<'_> {
    let ticket = lease.register(Some(&json!(id)));
    lease
        .child_lease(ticket, || LeaseRole::Exclusive)
        .ok()
        .expect("lent")
}

#[test]
fn concurrent_calls_under_an_inherited_lease_take_turns_and_a_waiting_child_never_sees_it() {
    let lease = &inherited();
    let first = lent(lease, 1);
    assert_eq!(lease_variable(&first), Some(Some("test-lease".to_owned())));
    let second = lease.register(Some(&json!(2)));
    std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let second = lease
                .child_lease(second, || LeaseRole::Exclusive)
                .ok()
                .expect("second call");
            let _ = sent.send(lease_variable(&second));
        });
        assert!(
            received.recv_timeout(WAITING).is_err(),
            "a second mutating call must not act under the inherited lease while the first child holds it"
        );
        drop(first);
        assert_eq!(
            received
                .recv_timeout(SETTLED)
                .expect("second call runs once the first ends"),
            Some(Some("test-lease".to_owned()))
        );
    });
}

#[test]
fn waiting_calls_take_the_lease_in_arrival_order() {
    let lease = &inherited();
    let first = lent(lease, 1);
    let second = lease.register(Some(&json!(2)));
    let third = lease.register(Some(&json!(3)));
    let (sent, received) = mpsc::channel();
    std::thread::scope(|scope| {
        for (ticket, id) in [(third, 3), (second, 2)] {
            let sent = sent.clone();
            scope.spawn(move || {
                let turn = lease
                    .child_lease(ticket, || LeaseRole::Exclusive)
                    .ok()
                    .expect("turn");
                let _ = sent.send(id);
                std::thread::sleep(Duration::from_millis(20));
                drop(turn);
            });
            std::thread::sleep(Duration::from_millis(50));
        }
        drop(first);
    });
    assert_eq!(received.try_iter().collect::<Vec<_>>(), [2, 3]);
}

#[test]
fn a_call_cancelled_before_it_queues_leaves_at_once_and_its_id_can_be_reused() {
    let lease = &inherited();
    let held = lent(lease, 1);
    let cancelled = lease.register(Some(&json!(2)));
    lease.cancel(&json!(2), || None);
    let left = std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let _ = sent.send(
                lease
                    .child_lease(cancelled, || LeaseRole::Exclusive)
                    .is_err(),
            );
        });
        let left = received.recv_timeout(WAITING);
        drop(held);
        left
    });
    assert_eq!(
        left,
        Ok(true),
        "a call cancelled before it queued must leave without waiting"
    );
    let reused = lease.register(Some(&json!(2)));
    assert!(matches!(
        lease.child_lease(reused, || LeaseRole::Exclusive),
        Ok(ChildLease::Lent(..))
    ));
}

#[test]
fn a_call_cancelled_while_waiting_never_gets_the_lease_and_leaves_the_slot_free() {
    let lease = inherited();
    let held = lent(&lease, 1);
    let waiting = lease.register(Some(&json!(2)));
    std::thread::scope(|scope| {
        let waiter = scope.spawn(|| lease.child_lease(waiting, || LeaseRole::Exclusive).is_err());
        std::thread::sleep(Duration::from_millis(50));
        lease.cancel(&json!(2), || None);
        assert!(
            waiter.join().expect("waiting call"),
            "cancelled call must not spawn"
        );
    });
    drop(held);
    assert!(matches!(lent(&lease, 3), ChildLease::Lent(..)));
}

#[test]
fn a_cancelled_holder_passes_the_lease_on_only_after_its_child_is_reaped() {
    let lease = &inherited();
    let (reaped, reaper_done) = mpsc::channel::<()>();
    let reaper = std::thread::spawn(move || {
        let _ = reaper_done.recv_timeout(SETTLED);
    });
    let holder = lent(lease, 1);
    let next = lease.register(Some(&json!(2)));
    std::thread::scope(|scope| {
        let (taken, turn) = mpsc::channel();
        let (output_closed, call_ends) = mpsc::channel::<()>();
        scope.spawn(move || {
            let _ = call_ends.recv_timeout(SETTLED);
            drop(holder);
        });
        scope.spawn(move || {
            let next = lease
                .child_lease(next, || LeaseRole::Exclusive)
                .ok()
                .expect("next call");
            let _ = taken.send(());
            drop(next);
        });
        lease.cancel(&json!(1), || {
            let _ = output_closed.send(());
            std::thread::sleep(Duration::from_millis(50));
            Some(reaper)
        });
        assert!(
            turn.recv_timeout(WAITING).is_err(),
            "the next call must wait while the cancelled child may still be releasing keys"
        );
        let _ = reaped.send(());
        assert!(turn.recv_timeout(SETTLED).is_ok());
    });
}

#[test]
fn a_shared_read_gets_the_lease_beside_the_holder_and_an_unused_command_gets_nothing() {
    let lease = inherited();
    let held = lent(&lease, 1);
    let ticket = lease.register(Some(&json!(2)));
    let read = lease
        .child_lease(ticket, || LeaseRole::Shared)
        .ok()
        .expect("shared read");
    assert_eq!(lease_variable(&read), Some(Some("test-lease".to_owned())));
    let ticket = lease.register(Some(&json!(3)));
    let unused = lease
        .child_lease(ticket, || LeaseRole::Unused)
        .ok()
        .expect("unused");
    assert_eq!(lease_variable(&unused), Some(None));
    drop(held);
    assert!(matches!(lent(&lease, 4), ChildLease::Lent(..)));
}

#[test]
fn a_call_without_an_id_still_waits_its_turn() {
    let lease = &inherited();
    let first = lent(lease, 1);
    let anonymous = lease.register(None);
    std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let turn = lease.child_lease(anonymous, || LeaseRole::Exclusive);
            let _ = sent.send(matches!(turn, Ok(ChildLease::Lent(..))));
        });
        assert!(received.recv_timeout(WAITING).is_err());
        drop(first);
        assert_eq!(received.recv_timeout(SETTLED), Ok(true));
    });
}

#[test]
fn a_call_thread_that_unwinds_before_its_turn_gives_up_its_place() {
    let lease = &inherited();
    let held = lent(lease, 1);
    let doomed = lease.register(Some(&json!(2)));
    let next = lease.register(Some(&json!(3)));
    let unwound = std::thread::scope(|scope| {
        scope
            .spawn(move || {
                let _place = QueuedTicket::new(lease, doomed);
                panic!("call thread unwinds before its turn");
            })
            .join()
    });
    assert!(unwound.is_err());
    drop(held);
    let turn = std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let turn = lease.child_lease(next, || LeaseRole::Exclusive);
            let _ = sent.send(matches!(turn, Ok(ChildLease::Lent(..))));
        });
        let turn = received.recv_timeout(WAITING);
        lease.forget(doomed);
        turn
    });
    assert_eq!(
        turn,
        Ok(true),
        "the call behind an unwound one must not wait forever"
    );
}

#[test]
fn an_unusable_inherited_value_refuses_exclusive_calls_at_once_and_runs_the_rest_without_it() {
    let lease = LeaseInheritance::Unusable {
        variable: TEST_VARIABLE,
        value: "2".to_owned(),
    };
    let Err(LeaseRefusal::Refused(AppError::Adapter(error))) =
        lease.child_lease(None, || LeaseRole::Exclusive)
    else {
        panic!("an exclusive call must be refused with an adapter error");
    };
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(error.message.contains("AGENT_DESKTOP_MCP_LEASE_TEST=2"));
    for role in [LeaseRole::Shared, LeaseRole::Unused] {
        let Ok(child) = lease.child_lease(None, || role) else {
            panic!("a {role:?} call must run without the unusable variable");
        };
        assert_eq!(lease_variable(&child), Some(None), "{role:?}");
    }
}

#[test]
fn an_exclusive_call_with_no_place_in_line_is_an_internal_refusal() {
    let lease = inherited();
    let Err(LeaseRefusal::Refused(AppError::Adapter(error))) =
        lease.child_lease(None, || LeaseRole::Exclusive)
    else {
        panic!("an exclusive call without a ticket must be refused with an answer");
    };
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn without_an_inherited_lease_children_are_left_to_take_it_themselves() {
    let owned = LeaseInheritance::Owned;
    assert_eq!(owned.register(Some(&json!(1))), None);
    for role in [
        LeaseRole::Exclusive,
        LeaseRole::Exclusive,
        LeaseRole::Shared,
    ] {
        let child = owned.child_lease(None, || role).ok().expect("owned call");
        assert_eq!(lease_variable(&child), None);
    }
}

#[cfg(unix)]
#[test]
fn only_a_child_the_lease_is_lent_to_inherits_its_descriptor() {
    use std::os::fd::AsRawFd;

    let path = std::env::temp_dir().join(format!(
        "agent-desktop-mcp-lease-test-{}",
        std::process::id()
    ));
    let file = std::fs::File::create(&path).expect("lease stand-in");
    let _ = std::fs::remove_file(&path);
    let descriptor = file.as_raw_fd();
    assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_SETFD, 0) }, 0);
    let protected = agent_desktop_core::protect_inherited_lease_descriptor(&descriptor.to_string());
    assert_eq!(protected, Some(descriptor));
    let lease = LeaseInheritance::Inherited(Box::new(
        InheritedLease::new(TEST_VARIABLE, descriptor.to_string().into())
            .with_descriptor(descriptor),
    ));
    let probe = |lease: &ChildLease<'_>, script: &str| {
        let script = script.replace("FD", &descriptor.to_string());
        let mut command = child_command(Path::new("sh"), &[], &words(&["-c", &script]), lease);
        command.output().expect("sh runs").status.success()
    };
    let holds = "[ -e /dev/fd/FD ] && [ \"$AGENT_DESKTOP_MCP_LEASE_TEST\" = FD ]";
    let lent_child = lent(&lease, 1);
    assert!(probe(&lent_child, holds));
    let ticket = lease.register(Some(&json!(2)));
    let shared = lease
        .child_lease(ticket, || LeaseRole::Shared)
        .ok()
        .expect("shared");
    assert!(probe(&shared, holds));
    let ticket = lease.register(Some(&json!(3)));
    let withheld = lease
        .child_lease(ticket, || LeaseRole::Unused)
        .ok()
        .expect("withheld");
    assert!(probe(
        &withheld,
        "[ ! -e /dev/fd/FD ] && [ -z \"${AGENT_DESKTOP_MCP_LEASE_TEST+x}\" ]"
    ));
}
