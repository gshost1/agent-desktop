use super::*;
use crate::AdapterError;
use serde_json::json;

#[test]
fn gone_ref_owners_carry_inspect_recovery_only_when_safe() {
    for details in [
        json!({ "kind": "resolve_owning_process_exited", "retryable": false }),
        json!({ "kind": "saved_ref_absent", "process_state": "exited", "retryable": false }),
        json!({ "kind": "saved_ref_absent", "process_state": "crashed", "retryable": false }),
    ] {
        for disposition in [
            DeliverySemantics::not_delivered(),
            DeliverySemantics::unknown(),
            DeliverySemantics::uncertain(),
            DeliverySemantics::delivered_unverified(),
            DeliverySemantics::delivered_verified(),
        ] {
            let error = AdapterError::stale_ref_because("owner is gone")
                .with_details(details.clone())
                .with_disposition(disposition);
            assert!(!error.permits_retry_by_default());
            let payload = ErrorPayload::from_app_error(&AppError::Adapter(error));
            assert_eq!(payload.details, Some(details.clone()));
            assert_eq!(payload.disposition, disposition);
            if disposition == DeliverySemantics::not_delivered() {
                let recovery = payload.recovery.expect("inspect the owner's state first");
                assert_eq!(recovery.strategy, "inspect_state_then_retry_original");
                assert!(recovery.retryable);
                assert!(recovery.requires_fresh_snapshot);
            } else {
                assert!(payload.recovery.is_none(), "{details}: {disposition:?}");
            }
        }
    }
}

#[test]
fn unreachable_ref_owner_without_exit_evidence_gets_no_recovery() {
    for state in [json!(null), json!("running"), json!("unresponsive")] {
        let error = AdapterError::stale_ref_because("owner is unreachable")
            .with_details(json!({
                "kind": "resolve_owner_unreachable",
                "process_state": state,
                "retryable": false,
            }))
            .with_disposition(DeliverySemantics::not_delivered());
        let payload = ErrorPayload::from_app_error(&AppError::Adapter(error));
        assert!(payload.recovery.is_none(), "{state}");
    }
}
