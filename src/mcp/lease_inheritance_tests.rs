use super::*;
use crate::mcp::call::child_command;
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
    let ticket = lease.register(&json!(id));
    lease.child_lease(ticket, || true).expect("lent")
}

#[test]
fn concurrent_calls_under_an_inherited_lease_take_turns_and_a_waiting_child_never_sees_it() {
    let lease = &inherited();
    let first = lent(lease, 1);
    assert_eq!(lease_variable(&first), Some(Some("test-lease".to_owned())));
    let second = lease.register(&json!(2));
    std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let second = lease.child_lease(second, || true).expect("second call");
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
    let second = lease.register(&json!(2));
    let third = lease.register(&json!(3));
    let (sent, received) = mpsc::channel();
    std::thread::scope(|scope| {
        for (ticket, id) in [(third, 3), (second, 2)] {
            let sent = sent.clone();
            scope.spawn(move || {
                let turn = lease.child_lease(ticket, || true).expect("turn");
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
    let cancelled = lease.register(&json!(2));
    lease.cancel(&json!(2), || None);
    let left = std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let _ = sent.send(lease.child_lease(cancelled, || true).is_none());
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
    let reused = lease.register(&json!(2));
    assert!(matches!(
        lease.child_lease(reused, || true),
        Some(ChildLease::Lent(..))
    ));
}

#[test]
fn a_call_cancelled_while_waiting_never_gets_the_lease_and_leaves_the_slot_free() {
    let lease = inherited();
    let held = lent(&lease, 1);
    let waiting = lease.register(&json!(2));
    std::thread::scope(|scope| {
        let waiter = scope.spawn(|| lease.child_lease(waiting, || true).is_none());
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
    let next = lease.register(&json!(2));
    std::thread::scope(|scope| {
        let (taken, turn) = mpsc::channel();
        let (output_closed, call_ends) = mpsc::channel::<()>();
        scope.spawn(move || {
            let _ = call_ends.recv_timeout(SETTLED);
            drop(holder);
        });
        scope.spawn(move || {
            let next = lease.child_lease(next, || true).expect("next call");
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
fn a_call_that_never_takes_the_lease_runs_at_once_without_it() {
    let lease = inherited();
    let held = lent(&lease, 1);
    let ticket = lease.register(&json!(2));
    let read_only = lease.child_lease(ticket, || false).expect("read-only call");
    assert_eq!(lease_variable(&read_only), Some(None));
    drop(read_only);
    drop(held);
}

#[test]
fn an_unusable_inherited_value_is_withheld_from_every_child() {
    let lease = LeaseInheritance::Unusable(TEST_VARIABLE);
    let first = lease.child_lease(None, || true).expect("call");
    let second = lease.child_lease(None, || true).expect("call");
    assert_eq!(lease_variable(&first), Some(None));
    assert_eq!(lease_variable(&second), Some(None));
}

#[test]
fn without_an_inherited_lease_children_are_left_to_take_it_themselves() {
    let owned = LeaseInheritance::Owned;
    assert_eq!(owned.register(&json!(1)), None);
    let first = owned.child_lease(None, || true).expect("owned call");
    let second = owned.child_lease(None, || true).expect("owned call");
    assert_eq!(lease_variable(&first), None);
    assert_eq!(lease_variable(&second), None);
}

#[test]
fn only_changes_and_permission_reports_wait_for_the_lease() {
    for argv in [
        &["list-apps"][..],
        &["version"],
        &["skills", "list"],
        &["clipboard-get"],
        &["snapshot"],
        &["find", "--role=button"],
        &["wait", "--text=Done"],
        &["screenshot"],
        &["list-surfaces"],
    ] {
        assert!(!needs_lease(&[], &words(argv)), "{argv:?}");
    }
    for argv in [
        &["status"][..],
        &["permissions"],
        &["click", "--", "@s8f3k2p9:e1"],
        &["press", "--", "ctrl+c"],
        &["clipboard-get", "--out=/tmp/x"],
        &["no-such-command"],
    ] {
        assert!(needs_lease(&[], &words(argv)), "{argv:?}");
    }
}

#[cfg(unix)]
#[test]
fn only_the_child_lent_the_lease_inherits_its_descriptor() {
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
    let lent_child = lent(&lease, 1);
    assert!(probe(
        &lent_child,
        "[ -e /dev/fd/FD ] && [ \"$AGENT_DESKTOP_MCP_LEASE_TEST\" = FD ]"
    ));
    let ticket = lease.register(&json!(2));
    let withheld = lease.child_lease(ticket, || false).expect("withheld");
    assert!(probe(
        &withheld,
        "[ ! -e /dev/fd/FD ] && [ -z \"${AGENT_DESKTOP_MCP_LEASE_TEST+x}\" ]"
    ));
}
