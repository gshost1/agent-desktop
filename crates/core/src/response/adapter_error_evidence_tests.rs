use crate::{AdapterError, AppError, DeliverySemantics, ErrorCode};

#[test]
fn a_refusal_after_a_focus_call_records_the_focus_beside_earlier_details() {
    let scrolled = AdapterError::timeout("x")
        .with_details(serde_json::json!({
            "preparatory_scroll": "may_have_changed_view",
            "retryable": true
        }))
        .after_focus();
    let bare = AdapterError::timeout("x")
        .with_details(serde_json::json!("cause"))
        .after_focus();

    assert_eq!(scrolled.disposition, DeliverySemantics::not_delivered());
    assert!(scrolled.is_explicitly_retryable());
    assert!(scrolled.moved_focus());
    let scrolled = scrolled.details.unwrap();
    assert_eq!(scrolled["preparatory_scroll"], "may_have_changed_view");
    assert_eq!(scrolled["preparatory_focus"], "may_have_moved_focus");
    let bare = bare.details.unwrap();
    assert_eq!(bare["error_details"], "cause");
    assert_eq!(bare["preparatory_focus"], "may_have_moved_focus");
}

#[test]
fn a_refusal_after_a_focus_call_keeps_stronger_evidence() {
    let error = AdapterError::timeout("x")
        .with_disposition(DeliverySemantics::uncertain())
        .after_focus();

    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    assert!(!AdapterError::timeout("x").unsent().moved_focus());
}

#[test]
fn a_permission_denied_io_error_keeps_its_code_as_an_adapter_error() {
    let denied = AppError::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "trace",
    ))
    .into_adapter_error();
    let broken = AppError::Io(std::io::Error::other("disk")).into_adapter_error();
    let adapter = AppError::Adapter(AdapterError::stale_ref("@e1")).into_adapter_error();

    assert_eq!(denied.code, ErrorCode::PermDenied);
    assert_eq!(broken.code, ErrorCode::Internal);
    assert_eq!(adapter.code, ErrorCode::StaleRef);
}
