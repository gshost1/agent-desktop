use super::*;

#[test]
fn only_timeout_errors_become_deadline_expiry_after_a_poll() {
    let clock = crate::wait::deadline::VirtualClock::freeze();
    let deadline = crate::Deadline::after(100).unwrap();
    let timeout = AdapterError::timeout("provider timeout");
    assert!(!expired_during_poll(&timeout, deadline));
    assert!(expired_during_poll(&deadline.timeout_error(), deadline));
    clock.advance(std::time::Duration::from_millis(100));
    assert!(expired_during_poll(&timeout, deadline));
    assert!(!expired_during_poll(
        &AdapterError::permission_denied(),
        deadline
    ));
}

#[test]
fn text_timeout_reports_the_char_count_and_never_the_raw_text() {
    let marker = "zzq93f_super_secret_marker_do_not_leak";

    let AppError::Adapter(error) = text(marker, 50, None, None).unwrap_err() else {
        panic!("adapter error expected")
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(!error.message.contains(marker));
    let details = error.details.expect("timeout details");
    assert!(!details.to_string().contains(marker));
    assert_eq!(details["predicate"], "text");
    assert_eq!(details["text_chars"], marker.chars().count());
}
