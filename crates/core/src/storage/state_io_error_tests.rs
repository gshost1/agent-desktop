use super::state_io_error;
use crate::ErrorCode;
use crate::storage::state_exposure::{exposed_directory, exposed_file, with_exposed_path};
use std::path::Path;

#[test]
fn a_state_directory_others_can_reach_is_perm_denied_with_its_path_and_fix() {
    let error = state_io_error(exposed_directory(Path::new("/state/snapshots")));

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert!(error.suggestion.unwrap().contains("chmod 700"));
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "state_directory_permission");
    assert_eq!(details["path"], "/state/snapshots");
    assert_eq!(details["retryable"], false);
}

#[test]
fn a_state_file_others_can_read_names_the_file_mode_fix() {
    let error = state_io_error(with_exposed_path(
        exposed_file(),
        Path::new("/state/a.lock"),
    ));

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert!(error.suggestion.unwrap().contains("chmod 600"));
    assert_eq!(error.details.unwrap()["path"], "/state/a.lock");
}

#[test]
fn an_exposure_with_no_known_path_omits_the_path() {
    let error = state_io_error(exposed_file());

    assert!(error.details.unwrap().get("path").is_none());
    assert!(!error.suggestion.unwrap().contains("details.path"));
}

#[test]
fn another_permission_refusal_gets_no_chmod_advice() {
    let refused = crate::storage::private_file::permission_denied(
        "private file parent is not owned by the effective user",
    );

    let error = state_io_error(refused);

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert!(error.suggestion.is_none());
    assert!(error.details.is_none());
}

#[test]
fn other_io_failures_stay_internal() {
    let error = state_io_error(std::io::Error::other("disk gone"));

    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.suggestion.is_none());
}
