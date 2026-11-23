use std::path::PathBuf;

use agent_desktop_core::ErrorCode;

use super::renderer_executable;

#[test]
fn a_renamed_cli_binary_starts_the_renderer_from_its_own_path() {
    let renamed = PathBuf::from("/opt/tools/deskref");

    let executable = renderer_executable(true, Ok(renamed.clone())).unwrap();

    assert_eq!(executable, renamed);
}

#[test]
fn a_process_that_never_ran_the_renderer_entry_refuses_as_not_supported() {
    let error = renderer_executable(false, Ok(PathBuf::from("/usr/bin/python3"))).unwrap_err();

    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("FFI host")),
        "{error:?}"
    );
}

#[test]
fn the_agent_desktop_name_alone_does_not_make_a_process_the_renderer_host() {
    let error =
        renderer_executable(false, Ok(PathBuf::from("/usr/local/bin/agent-desktop"))).unwrap_err();

    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
}

#[test]
fn a_test_binary_has_not_claimed_the_renderer_role() {
    let error = super::executable().unwrap_err();

    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
}
