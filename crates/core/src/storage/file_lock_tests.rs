use super::state_io_error;
use crate::ErrorCode;

#[test]
fn a_state_directory_others_can_read_is_perm_denied_with_its_fix() {
    let refused = crate::storage::private_file::permission_denied(
        "private file parent is accessible by group or other users",
    );

    let error = state_io_error(refused);

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert!(error.suggestion.unwrap().contains("chmod 700"));
    assert_eq!(error.details.unwrap()["kind"], "state_directory_permission");
}

#[test]
fn other_io_failures_stay_internal() {
    let error = state_io_error(std::io::Error::other("disk gone"));

    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.suggestion.is_none());
}
