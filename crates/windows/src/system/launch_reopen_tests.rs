use super::*;

#[test]
fn reopen_is_refused_because_windows_has_no_reopen_event() {
    let options = LaunchOptions {
        reopen: true,
        ..Default::default()
    };
    let error = validate_launch_options(&options).expect_err("reopen is unsupported");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}
