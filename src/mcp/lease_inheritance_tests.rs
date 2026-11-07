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
    LeaseInheritance::Inherited(InheritedLease::new(TEST_VARIABLE, "test-lease".into()))
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

#[test]
fn concurrent_calls_under_an_inherited_lease_take_turns_and_a_waiting_child_never_sees_it() {
    let lease = &inherited();
    let first = lease.child_lease(&json!(1), || true).expect("first call");
    assert_eq!(lease_variable(&first), Some(Some("test-lease".to_owned())));
    std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let second = lease.child_lease(&json!(2), || true).expect("second call");
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
fn a_call_that_never_takes_the_lease_runs_at_once_without_it() {
    let lease = inherited();
    let held = lease.child_lease(&json!(1), || true).expect("first call");
    let read_only = lease
        .child_lease(&json!(2), || false)
        .expect("read-only call");
    assert_eq!(lease_variable(&read_only), Some(None));
    drop(read_only);
    drop(held);
}

#[test]
fn a_call_cancelled_while_waiting_never_gets_the_lease_and_leaves_the_slot_free() {
    let lease = inherited();
    let held = lease.child_lease(&json!(1), || true).expect("first call");
    std::thread::scope(|scope| {
        let waiting = scope.spawn(|| lease.child_lease(&json!(2), || true).is_none());
        while !waiting.is_finished() {
            lease.cancel(&json!(2));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            waiting.join().expect("waiting call"),
            "cancelled call must not spawn"
        );
    });
    drop(held);
    let next = lease.child_lease(&json!(3), || true);
    assert!(matches!(next, Some(ChildLease::Lent(_))));
}

#[test]
fn without_an_inherited_lease_children_are_left_to_take_it_themselves() {
    let owned = LeaseInheritance::Owned;
    let first = owned.child_lease(&json!(1), || true).expect("owned call");
    let second = owned.child_lease(&json!(2), || true).expect("owned call");
    assert_eq!(lease_variable(&first), None);
    assert_eq!(lease_variable(&second), None);
}

#[test]
fn only_commands_that_neither_change_anything_nor_need_a_permission_skip_the_lease() {
    for argv in [
        &["list-apps"][..],
        &["list-windows"],
        &["version"],
        &["status"],
        &["skills", "list"],
        &["clipboard-get"],
    ] {
        assert!(!needs_lease(&[], &words(argv)), "{argv:?}");
    }
    for argv in [
        &["snapshot"][..],
        &["click", "--", "@s8f3k2p9:e1"],
        &["press", "--", "ctrl+c"],
        &["clipboard-get", "--out=/tmp/x"],
        &["permissions", "--request"],
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
    let lease = LeaseInheritance::Inherited(InheritedLease::new(
        TEST_VARIABLE,
        descriptor.to_string().into(),
    ));
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    assert_ne!(
        flags & libc::FD_CLOEXEC,
        0,
        "the server keeps it from children"
    );
    let probe = |lease: &ChildLease<'_>, script: &str| {
        let script = script.replace("FD", &descriptor.to_string());
        let mut command = child_command(Path::new("sh"), &[], &words(&["-c", &script]), lease);
        command.output().expect("sh runs").status.success()
    };
    let lent = lease.child_lease(&json!(1), || true).expect("lent");
    assert!(probe(
        &lent,
        "[ -e /dev/fd/FD ] && [ \"$AGENT_DESKTOP_MCP_LEASE_TEST\" = FD ]"
    ));
    let withheld = lease.child_lease(&json!(2), || false).expect("withheld");
    assert!(probe(
        &withheld,
        "[ ! -e /dev/fd/FD ] && [ -z \"${AGENT_DESKTOP_MCP_LEASE_TEST+x}\" ]"
    ));
}
