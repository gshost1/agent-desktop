use super::*;
use agent_desktop_core::system::launch_presentation::LaunchPresentation;

#[test]
fn reopen_is_refused_because_windows_has_no_reopen_event() {
    let options = LaunchOptions {
        presentation: LaunchPresentation {
            reopen: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let error = validate_launch_options(&options).expect_err("reopen is unsupported");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
}
