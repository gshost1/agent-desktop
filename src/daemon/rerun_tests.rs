use super::is_undelivered_permission_refusal;

fn envelope(code: &str, delivery: &str) -> Vec<u8> {
    format!(
        r#"{{"version":"2.7","ok":false,"command":"snapshot","error":{{"code":"{code}","message":"m","disposition":{{"delivery":"{delivery}","retry":"safe"}}}}}}"#
    )
    .into_bytes()
}

#[test]
fn undelivered_permission_refusal_is_rerun() {
    let mut stdout = envelope("PERM_DENIED", "not_delivered");
    stdout.push(b'\n');

    assert!(is_undelivered_permission_refusal(&stdout));
}

#[test]
fn other_errors_and_delivered_refusals_are_kept() {
    assert!(!is_undelivered_permission_refusal(&envelope(
        "APP_NOT_FOUND",
        "not_delivered"
    )));
    assert!(!is_undelivered_permission_refusal(&envelope(
        "PERM_DENIED",
        "delivery_uncertain"
    )));
    assert!(!is_undelivered_permission_refusal(
        br#"{"version":"2.7","ok":true,"command":"status","data":{}}"#
    ));
    assert!(!is_undelivered_permission_refusal(b"not json"));
}
