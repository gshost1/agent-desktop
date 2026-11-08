use super::*;
use crate::mcp::lease_inheritance::LeaseInheritance;
use crate::mcp::lease_role::LeaseRole;
use serde_json::json;
use std::sync::mpsc;

const BOUND: Duration = Duration::from_millis(100);
const SETTLED: Duration = Duration::from_secs(5);

fn bounded() -> LeaseInheritance {
    LeaseInheritance::Inherited(Box::new(
        InheritedLease::new("AGENT_DESKTOP_MCP_LEASE_TEST", "test-lease".into())
            .with_queue_bound(BOUND),
    ))
}

#[test]
fn an_exclusive_call_waits_at_most_the_queue_bound_then_times_out_undelivered() {
    let lease = &bounded();
    let first = lease.register(Some(&json!(1)));
    let held = lease
        .child_lease(first, || LeaseRole::Exclusive)
        .ok()
        .expect("first call holds the lease");
    let waiting = lease.register(Some(&json!(2)));
    let refusal = std::thread::scope(|scope| {
        let (sent, received) = mpsc::channel();
        scope.spawn(move || {
            let refusal = match lease.child_lease(waiting, || LeaseRole::Exclusive) {
                Err(LeaseRefusal::Refused(AppError::Adapter(error))) => Some(error),
                _ => None,
            };
            let _ = sent.send(refusal);
        });
        let refusal = received.recv_timeout(SETTLED);
        drop(held);
        refusal
    });
    let error = refusal
        .expect("a waiting call must give up within its bound")
        .expect("refused with an adapter error");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.message.contains("held by an earlier call"));
    let next = lease.register(Some(&json!(3)));
    assert!(
        matches!(
            lease.child_lease(next, || LeaseRole::Exclusive),
            Ok(ChildLease::Lent(..))
        ),
        "a timed-out call must leave the line"
    );
}
