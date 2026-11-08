use super::*;
use crate::mcp::inherited_lease::InheritedLease;
use crate::mcp::lease_role::LeaseRole;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn a_call_thread_that_never_starts_gives_up_its_place_in_line() {
    let lease = Arc::new(LeaseInheritance::Inherited(Box::new(
        InheritedLease::new("AGENT_DESKTOP_MCP_LEASE_TEST", "test-lease".into())
            .with_queue_bound(Duration::from_millis(200)),
    )));
    let first = lease.register(Some(&json!(1)));
    let place = QueuedTicket::new(Arc::clone(&lease), first);
    let started = place.run_on(
        |body| {
            drop(body);
            Err::<(), _>(std::io::Error::other("no thread"))
        },
        |_| {},
    );
    assert!(started.is_err());
    let next = lease.register(Some(&json!(2)));
    assert!(
        lease.child_lease(next, || LeaseRole::Exclusive).is_ok(),
        "the call behind one whose thread never started must not wait for it"
    );
}
