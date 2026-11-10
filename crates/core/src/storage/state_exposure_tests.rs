use super::{exposed, find, with_exposed_path};
use std::path::Path;

#[test]
fn an_exposure_keeps_the_refusal_text_callers_already_read() {
    let error = exposed(Some(Path::new("/state")), true);

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        error.to_string(),
        "private file parent is accessible by group or other users"
    );
    assert_eq!(
        exposed(None, false).to_string(),
        "private file is accessible by group or other users"
    );
}

#[test]
fn a_file_exposure_learns_its_path_from_the_opener() {
    let error = with_exposed_path(exposed(None, false), Path::new("/state/refs.lock"));

    let exposure = find(&error).expect("still an exposure");
    assert_eq!(
        exposure.path.as_deref(),
        Some(Path::new("/state/refs.lock"))
    );
    assert!(!exposure.directory);
}

#[test]
fn a_known_path_and_other_errors_pass_through_unchanged() {
    let named = with_exposed_path(
        exposed(Some(Path::new("/state")), true),
        Path::new("/other"),
    );
    assert_eq!(
        find(&named).unwrap().path.as_deref(),
        Some(Path::new("/state"))
    );

    let plain = with_exposed_path(std::io::Error::other("disk gone"), Path::new("/state"));
    assert!(find(&plain).is_none());
}
