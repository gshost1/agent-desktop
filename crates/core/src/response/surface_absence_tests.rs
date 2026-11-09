use crate::{AdapterError, ErrorCode, SnapshotSurface};

fn closed_menu() -> AdapterError {
    AdapterError::surface_absent(SnapshotSurface::Menu, "no open menu")
}

#[test]
fn a_confirmed_closed_menu_makes_a_menu_ref_stale() {
    let error = closed_menu().for_saved_ref_surface(SnapshotSurface::Menu);

    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "menu_closed");
    assert_eq!(details["retryable"], false);
}

#[test]
fn an_unconfirmed_menu_lookup_is_returned_as_it_is() {
    let timeout = AdapterError::timeout("The menu inventory could not be read in full");
    assert_eq!(
        timeout.for_saved_ref_surface(SnapshotSurface::Menu).code,
        ErrorCode::Timeout
    );

    let bare = AdapterError::new(ErrorCode::ElementNotFound, "no open menu");
    assert_eq!(
        bare.for_saved_ref_surface(SnapshotSurface::Menu).code,
        ErrorCode::ElementNotFound
    );
}

#[test]
fn another_surfaces_absence_is_returned_as_it_is() {
    let sheet = AdapterError::surface_absent(SnapshotSurface::Sheet, "no sheet");
    let error = sheet.for_saved_ref_surface(SnapshotSurface::Sheet);

    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert!(error.is_confirmed_surface_absence(SnapshotSurface::Sheet));
}
